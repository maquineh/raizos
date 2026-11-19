//! Slot estatico de observacao de uma instancia exata, separado da FIFO IPC.
use crate::controle::{retirar_atual, CaixaMensagens};
use contrato_sistema::{erro, evento_vida};
use contrato_sistema::{Mensagem, QUANTIDADE_SERVICOS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventoVida {
    pub referencia: u64,
    pub tipo: u64,
    pub estado: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct Observacao {
    referencia: u64,
    evento: Option<EventoVida>,
    ultimo_estado: u64,
}

#[derive(Clone, Copy, Debug)]
pub enum EventoEscolhido {
    Ipc(Mensagem),
    Vida(EventoVida),
}

/// Rotacao entre classes elegiveis; eventos de vida ocupam slot independente.
pub fn selecionar(
    fila: &mut CaixaMensagens,
    obs: &mut Observacao,
    preferir_vida: &mut bool,
    atuais: &[u64; QUANTIDADE_SERVICOS],
) -> Option<EventoEscolhido> {
    if *preferir_vida {
        if let Some(evento) = obs.consumir() {
            *preferir_vida = false;
            return Some(EventoEscolhido::Vida(evento));
        }
    }
    if let Some(mensagem) = retirar_atual(fila, atuais) {
        *preferir_vida = true;
        return Some(EventoEscolhido::Ipc(mensagem));
    }
    if let Some(evento) = obs.consumir() {
        *preferir_vida = false;
        return Some(EventoEscolhido::Vida(evento));
    }
    None
}
impl Observacao {
    pub const NOVA: Self = Self {
        referencia: 0,
        evento: None,
        ultimo_estado: 0,
    };
    pub fn inscrever(&mut self, referencia: u64, estado: u64) -> Result<u64, u64> {
        if self.referencia != 0 {
            return Err(if self.referencia == referencia {
                erro::ESTADO
            } else {
                erro::RECURSOS
            });
        }
        self.referencia = referencia;
        self.ultimo_estado = estado;
        self.evento = None;
        Ok(estado)
    }
    pub fn cancelar(&mut self, referencia: u64) -> Result<(), u64> {
        if self.referencia == 0 {
            return Err(erro::ESTADO);
        }
        if self.referencia != referencia {
            return Err(erro::REFERENCIA_ANTIGA);
        }
        *self = Self::NOVA;
        Ok(())
    }
    pub fn inscrita(&self) -> bool {
        self.referencia != 0
    }
    pub fn pendente(&self) -> bool {
        self.evento.is_some()
    }
    pub fn publicar_estado(&mut self, referencia: u64, estado: u64) -> bool {
        if self.referencia != referencia
            || referencia == 0
            || self.ultimo_estado == estado
            || self
                .evento
                .is_some_and(|e| e.tipo == evento_vida::INVALIDACAO)
        {
            return false;
        }
        self.ultimo_estado = estado;
        self.evento = Some(EventoVida {
            referencia,
            tipo: evento_vida::ESTADO,
            estado,
        });
        true
    }
    pub fn invalidar(&mut self, referencia: u64) -> bool {
        if self.referencia != referencia
            || referencia == 0
            || self
                .evento
                .is_some_and(|e| e.tipo == evento_vida::INVALIDACAO)
        {
            return false;
        }
        self.evento = Some(EventoVida {
            referencia,
            tipo: evento_vida::INVALIDACAO,
            estado: self.ultimo_estado,
        });
        true
    }
    pub fn consumir(&mut self) -> Option<EventoVida> {
        let evento = self.evento.take()?;
        if evento.tipo == evento_vida::INVALIDACAO {
            *self = Self::NOVA;
        }
        Some(evento)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use contrato_sistema::{criar_referencia, estado, REDE, SUPERVISOR, TERMINAL};
    fn r(id: u64, geracao: u64) -> u64 {
        criar_referencia(id, geracao).unwrap()
    }
    #[test]
    fn estado_inicial_coalescencia_e_invalidez_terminal() {
        let (antiga, nova) = (r(REDE, 1), r(REDE, 2));
        let mut o = Observacao::NOVA;
        assert_eq!(o.inscrever(antiga, estado::FALHOU), Ok(estado::FALHOU));
        assert!(!o.pendente());
        assert!(!o.publicar_estado(antiga, estado::FALHOU));
        assert_eq!(o.inscrever(antiga, estado::FALHOU), Err(erro::ESTADO));
        assert!(o.publicar_estado(antiga, estado::FINALIZADO));
        assert!(o.invalidar(antiga));
        assert!(!o.invalidar(antiga));
        assert!(!o.publicar_estado(antiga, estado::EXECUTAVEL));
        assert_eq!(o.inscrever(nova, estado::EXECUTAVEL), Err(erro::RECURSOS));
        assert_eq!(
            o.consumir(),
            Some(EventoVida {
                referencia: antiga,
                tipo: evento_vida::INVALIDACAO,
                estado: estado::FINALIZADO
            })
        );
        assert_eq!(
            o.inscrever(nova, estado::EXECUTAVEL),
            Ok(estado::EXECUTAVEL)
        );
        assert!(!o.invalidar(antiga));
        assert!(!o.pendente());
    }
    #[test]
    fn cancelamento_exato_libera_slot_ate_para_referencia_antiga() {
        let mut o = Observacao::NOVA;
        let (antiga, nova) = (r(REDE, 1), r(REDE, 2));
        o.inscrever(antiga, estado::EXECUTAVEL).unwrap();
        o.invalidar(antiga);
        assert_eq!(o.cancelar(nova), Err(erro::REFERENCIA_ANTIGA));
        assert!(o.pendente());
        assert_eq!(o.cancelar(antiga), Ok(()));
        assert_eq!(
            o.inscrever(nova, estado::EXECUTAVEL),
            Ok(estado::EXECUTAVEL)
        );
        assert_eq!(o.cancelar(nova), Ok(()));
        assert_eq!(o.cancelar(nova), Err(erro::ESTADO));
    }
    #[test]
    fn vida_nao_compete_por_espaco_e_classes_alternam() {
        use crate::controle::CAPACIDADE;
        let (terminal, rede, supervisor) = (r(TERMINAL, 1), r(REDE, 1), r(SUPERVISOR, 1));
        let atuais = [terminal, rede, supervisor];
        let mut o = Observacao::NOVA;
        let mut f = CaixaMensagens::NOVA;
        let mut preferir_vida = false;
        o.inscrever(rede, estado::EXECUTAVEL).unwrap();
        for n in 0..CAPACIDADE {
            f.inserir(Mensagem {
                origem: terminal,
                codigo: evento_vida::ESTADO,
                a: n as u64,
                ..Mensagem::default()
            })
            .unwrap();
        }
        assert_eq!(f.inserir(Mensagem::default()), Err(erro::FILA_CHEIA));
        assert!(o.publicar_estado(rede, estado::FALHOU));
        assert!(
            matches!(selecionar(&mut f,&mut o,&mut preferir_vida,&atuais),Some(EventoEscolhido::Ipc(m)) if m.a==0)
        );
        assert!(
            matches!(selecionar(&mut f,&mut o,&mut preferir_vida,&atuais),Some(EventoEscolhido::Vida(v)) if v.estado==estado::FALHOU)
        );
        assert_eq!(f.tamanho(), CAPACIDADE - 1);
    }
    #[test]
    fn timeout_preserva_ambas_fontes_e_ultima_vida_antes_da_troca() {
        use crate::agendamento::{expirar_recepcao, EsperaIpc, Estado};
        let (terminal, rede, supervisor) = (r(TERMINAL, 1), r(REDE, 1), r(SUPERVISOR, 1));
        let atuais = [terminal, rede, supervisor];
        let mut o = Observacao::NOVA;
        let mut f = CaixaMensagens::NOVA;
        let mut estado_exec = Estado::ATIVO;
        let mut espera = EsperaIpc::NOVA;
        let mut pendente = true;
        let mut limite = Some(10);
        let mut preferir_vida = true;
        o.inscrever(rede, estado::EXECUTAVEL).unwrap();
        espera.registrar(&mut estado_exec, 0).unwrap();
        f.inserir(Mensagem {
            origem: terminal,
            codigo: 1,
            ..Mensagem::default()
        })
        .unwrap();
        assert!(o.invalidar(rede));
        assert!(expirar_recepcao(
            10,
            &mut limite,
            &mut pendente,
            &mut espera,
            &mut estado_exec
        ));
        assert!(o.pendente());
        assert_eq!(f.tamanho(), 1);
        assert!(
            matches!(selecionar(&mut f,&mut o,&mut preferir_vida,&atuais), Some(EventoEscolhido::Vida(v)) if v.tipo==evento_vida::INVALIDACAO && v.estado==estado::EXECUTAVEL)
        );
        assert_eq!(f.tamanho(), 1);
        o.inscrever(rede, estado::FALHOU).unwrap();
        assert!(o.invalidar(rede));
        assert_eq!(o.consumir().unwrap().estado, estado::FALHOU);
    }
}
