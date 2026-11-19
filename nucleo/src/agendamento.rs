//! Vida e disponibilidade de uma instancia, independentes da ABI publica.
use contrato_sistema::estado;

/// Comparacao modular valida se a expiracao for tratada antes de decorrerem
/// 2^63 tiques observados apos o prazo, conforme ADR 003.
pub fn prazo_vencido(agora: u64, prazo: u64) -> bool {
    agora.wrapping_sub(prazo) < (1u64 << 63)
}

/// Valida campos antes de consultar ou consumir a fila.
pub fn calcular_prazo(modo: u64, duracao: u64, agora: u64) -> Result<Option<u64>, u64> {
    use contrato_sistema::{erro, prazo};
    match (modo, duracao) {
        (prazo::IMEDIATO | prazo::SEM_PRAZO, 0) => Ok(None),
        (prazo::FINITO, n) if (1..=(i64::MAX as u64)).contains(&n) => {
            Ok(Some(agora.wrapping_add(n)))
        }
        _ => Err(erro::CHAMADA),
    }
}

/// Conclui uma recepcao ainda pendente, inclusive quando ja pronta pelo IPC.
/// Nao acessa a fila; o chamador grava o timeout no frame antes de seleciona-la.
pub fn expirar_recepcao(
    agora: u64,
    prazo: &mut Option<u64>,
    pendente: &mut bool,
    espera: &mut EsperaIpc,
    estado: &mut Estado,
) -> bool {
    if !*pendente || !estado.ativo() || !prazo.is_some_and(|p| prazo_vencido(agora, p)) {
        return false;
    }
    *pendente = false;
    *prazo = None;
    espera.cancelar();
    estado.despertar();
    true
}

/// Desvincula toda espera da instancia antes de finalizar ou substituir.
/// O contexto revogado nao recebe um resultado nem executa novamente.
pub fn cancelar_espera(
    espera: &mut EsperaIpc,
    recepcao_pendente: &mut bool,
    prazo: &mut Option<u64>,
) {
    espera.cancelar();
    *recepcao_pendente = false;
    *prazo = None;
}

/// Conclui somente a espera one-shot vinculada a referencia exata. O chamador
/// processa antes o prazo e grava o resultado em contexto ainda publicado.
pub fn invalidar_espera_por_alvo(
    alvo: u64,
    observado: &mut u64,
    espera: &mut EsperaIpc,
    pendente: &mut bool,
    prazo: &mut Option<u64>,
    estado: &mut Estado,
) -> bool {
    if alvo == 0 || *observado != alvo || !*pendente || !estado.ativo() {
        return false;
    }
    cancelar_espera(espera, pendente, prazo);
    *observado = 0;
    estado.despertar();
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vida {
    Inexistente,
    Ativa,
    Falhou,
    Finalizada,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agendamento {
    Pronto,
    Bloqueado,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Estado {
    pub vida: Vida,
    pub agendamento: Agendamento,
}

/// Espera pela disponibilidade da caixa propria. A mensagem permanece na FIFO
/// ate RECEBER, podendo ser purgada se sua origem for revogada antes disso.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EsperaIpc {
    pendente: bool,
}
impl EsperaIpc {
    pub const NOVA: Self = Self { pendente: false };

    /// A consulta da fila e a transicao devem ocorrer sob a mesma exclusao
    /// mutua que a insercao (IF=0 na unica CPU do kernel).
    pub fn registrar(&mut self, estado: &mut Estado, mensagens: usize) -> Result<bool, u64> {
        if self.pendente || !estado.executavel() {
            return Err(contrato_sistema::erro::ESTADO);
        }
        if mensagens != 0 {
            return Ok(false);
        }
        debug_assert!(estado.executavel());
        estado.bloquear();
        self.pendente = true;
        Ok(true)
    }

    /// Somente uma insercao admitida pode concluir a espera.
    pub fn mensagem_admitida(&mut self, estado: &mut Estado) -> bool {
        if !self.pendente {
            return false;
        }
        self.pendente = false;
        estado.despertar()
    }

    pub fn cancelar(&mut self) {
        self.pendente = false;
    }
}
impl Estado {
    pub const VAZIO: Self = Self {
        vida: Vida::Inexistente,
        agendamento: Agendamento::Pronto,
    };
    pub const ATIVO: Self = Self {
        vida: Vida::Ativa,
        agendamento: Agendamento::Pronto,
    };
    pub fn executavel(self) -> bool {
        self.vida == Vida::Ativa && self.agendamento == Agendamento::Pronto
    }
    pub fn ativo(self) -> bool {
        self.vida == Vida::Ativa
    }
    pub fn abi(self) -> u64 {
        match self.vida {
            Vida::Inexistente => estado::INEXISTENTE,
            Vida::Ativa => estado::EXECUTAVEL,
            Vida::Falhou => estado::FALHOU,
            Vida::Finalizada => estado::FINALIZADO,
        }
    }
    pub fn bloquear(&mut self) -> bool {
        if self.executavel() {
            self.agendamento = Agendamento::Bloqueado;
            true
        } else {
            false
        }
    }
    pub fn despertar(&mut self) -> bool {
        if self.ativo() && self.agendamento == Agendamento::Bloqueado {
            self.agendamento = Agendamento::Pronto;
            true
        } else {
            false
        }
    }
    pub fn terminar(&mut self, vida: Vida) {
        debug_assert!(matches!(vida, Vida::Falhou | Vida::Finalizada));
        self.vida = vida;
        self.agendamento = Agendamento::Pronto;
    }
}
#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn prazo_valida_modos_limites_e_volta_do_contador() {
        use contrato_sistema::{erro, prazo};
        assert_eq!(calcular_prazo(prazo::IMEDIATO, 0, 7), Ok(None));
        assert_eq!(calcular_prazo(prazo::SEM_PRAZO, 0, 7), Ok(None));
        assert_eq!(calcular_prazo(prazo::FINITO, 0, 7), Err(erro::CHAMADA));
        assert_eq!(calcular_prazo(prazo::IMEDIATO, 1, 7), Err(erro::CHAMADA));
        assert_eq!(calcular_prazo(prazo::SEM_PRAZO, 1, 7), Err(erro::CHAMADA));
        assert_eq!(calcular_prazo(3, 0, 7), Err(erro::CHAMADA));
        assert_eq!(
            calcular_prazo(prazo::FINITO, 1u64 << 63, 7),
            Err(erro::CHAMADA)
        );
        assert_eq!(
            calcular_prazo(prazo::FINITO, i64::MAX as u64, 7),
            Ok(Some(7u64.wrapping_add(i64::MAX as u64)))
        );
        assert_eq!(calcular_prazo(prazo::FINITO, 3, u64::MAX - 1), Ok(Some(1)));
        assert!(!prazo_vencido(u64::MAX, 1));
        assert!(!prazo_vencido(0, 1));
        assert!(prazo_vencido(1, 1));
        assert!(prazo_vencido(2, 1));
        assert!(!prazo_vencido(1u64 << 63, 0));
    }
    #[test]
    fn timeout_conclui_uma_vez_mesmo_apos_despertar_por_mensagem() {
        let mut estado = Estado::ATIVO;
        let mut espera = EsperaIpc::NOVA;
        let mut pendente = true;
        let mut prazo = Some(5);
        assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
        assert!(espera.mensagem_admitida(&mut estado));
        assert!(!expirar_recepcao(
            4,
            &mut prazo,
            &mut pendente,
            &mut espera,
            &mut estado
        ));
        assert!(expirar_recepcao(
            5,
            &mut prazo,
            &mut pendente,
            &mut espera,
            &mut estado
        ));
        assert!(!expirar_recepcao(
            6,
            &mut prazo,
            &mut pendente,
            &mut espera,
            &mut estado
        ));
        assert!(estado.executavel());
        assert!(!pendente);
        assert_eq!(prazo, None);
    }
    #[test]
    fn expiracao_nao_modifica_nova_instancia_ou_resultado_ja_concluido() {
        let mut estado = Estado::ATIVO;
        let mut espera = EsperaIpc::NOVA;
        let mut pendente = false;
        let mut prazo = None;
        assert!(!expirar_recepcao(
            123,
            &mut prazo,
            &mut pendente,
            &mut espera,
            &mut estado
        ));
        assert!(estado.executavel());
        pendente = true;
        prazo = Some(5);
        estado.terminar(Vida::Finalizada);
        assert!(!expirar_recepcao(
            5,
            &mut prazo,
            &mut pendente,
            &mut espera,
            &mut estado
        ));
        // Novo contexto substitui todos os campos da instancia encerrada;
        // nao existe callback externo que carregue o prazo antigo.
        estado = Estado::ATIVO;
        espera = EsperaIpc::NOVA;
        pendente = true;
        prazo = Some(10);
        assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
        assert!(!expirar_recepcao(
            5,
            &mut prazo,
            &mut pendente,
            &mut espera,
            &mut estado
        ));
        assert!(!estado.executavel());
        assert!(pendente);
        assert_eq!(prazo, Some(10));
        assert!(expirar_recepcao(
            10,
            &mut prazo,
            &mut pendente,
            &mut espera,
            &mut estado
        ));
    }
    #[test]
    fn bloqueio_nao_altera_vida_e_despertar_so_ocorre_uma_vez() {
        let mut e = Estado::ATIVO;
        assert!(e.bloquear());
        assert!(e.ativo());
        assert!(!e.executavel());
        assert_eq!(e.abi(), estado::EXECUTAVEL);
        assert!(!e.bloquear());
        assert!(e.despertar());
        assert!(!e.despertar());
        assert!(e.executavel());
    }
    #[test]
    fn morte_cancela_disponibilidade_e_impede_despertar() {
        for vida in [Vida::Falhou, Vida::Finalizada] {
            let mut e = Estado::ATIVO;
            assert!(e.bloquear());
            e.terminar(vida);
            assert!(!e.ativo());
            assert!(!e.executavel());
            assert!(!e.despertar());
            assert!(!e.bloquear());
        }
    }
    #[test]
    fn espera_observa_fila_e_conclui_uma_unica_vez() {
        let mut estado = Estado::ATIVO;
        let mut espera = EsperaIpc::NOVA;
        assert_eq!(espera.registrar(&mut estado, 1), Ok(false));
        assert!(estado.executavel());
        assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
        assert_eq!(
            espera.registrar(&mut estado, 0),
            Err(contrato_sistema::erro::ESTADO)
        );
        assert!(espera.mensagem_admitida(&mut estado));
        assert!(!espera.mensagem_admitida(&mut estado));
        assert!(estado.executavel());
    }
    #[test]
    fn espera_cancelada_na_morte_nao_acorda_nova_instancia() {
        let mut estado = Estado::ATIVO;
        let mut espera = EsperaIpc::NOVA;
        assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
        espera.cancelar();
        estado.terminar(Vida::Finalizada);
        assert!(!espera.mensagem_admitida(&mut estado));
        estado = Estado::ATIVO;
        assert!(!espera.mensagem_admitida(&mut estado));
    }
    #[test]
    fn fila_admitida_acorda_sem_consumir_e_purga_ainda_pode_revogar() {
        use crate::controle::CaixaMensagens;
        use contrato_sistema::Mensagem;
        let mut fila = CaixaMensagens::NOVA;
        let mut estado = Estado::ATIVO;
        let mut espera = EsperaIpc::NOVA;
        let m = Mensagem {
            origem: 0x100000002,
            codigo: 1,
            a: 1,
            b: 0,
            c: 0,
            d: 0,
        };
        assert_eq!(espera.registrar(&mut estado, fila.tamanho()), Ok(true));
        fila.inserir(m).unwrap();
        assert!(espera.mensagem_admitida(&mut estado));
        fila.inserir(Mensagem { a: 2, ..m }).unwrap();
        assert!(!espera.mensagem_admitida(&mut estado));
        assert_eq!(fila.retirar().unwrap().a, 1);
        assert_eq!(fila.retirar().unwrap().a, 2);
        assert_eq!(espera.registrar(&mut estado, fila.tamanho()), Ok(true));
        fila.inserir(m).unwrap();
        assert!(espera.mensagem_admitida(&mut estado));
        assert_eq!(fila.descartar_origem(m.origem), 1);
        assert!(fila.retirar().is_none());
        assert_eq!(espera.registrar(&mut estado, fila.tamanho()), Ok(true));
    }
}
