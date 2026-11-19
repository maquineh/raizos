//! Slot limitado de IRQ exclusiva, acessado com IF=0.
use contrato_sistema::erro;

#[derive(Clone, Copy)]
pub struct Irq {
    pub referencia: u64,
    pendente: bool,
    entregue: u64,
    proxima: u64,
}
impl Irq {
    pub const NOVA: Self = Self {
        referencia: 0,
        pendente: false,
        entregue: 0,
        proxima: 1,
    };
    pub fn armar(&mut self, referencia: u64) -> Result<(), u64> {
        if self.referencia != 0 {
            return Err(erro::ESTADO);
        }
        self.referencia = referencia;
        Ok(())
    }
    pub fn publicar(&mut self) {
        if self.referencia != 0 {
            self.pendente = true;
        }
    }
    /// IF=0 ou leitura exclusiva: consulta sem retirar nem alterar o slot.
    #[cfg(any(feature = "teste-seletor-irq-ipc", feature = "teste-irq-fifo-cheia"))]
    pub fn selecionavel(&self, referencia: u64) -> bool {
        self.referencia == referencia && self.pendente && self.entregue == 0
    }
    /// IF=0: slot associado e sem notificação pendente ou entregue.
    #[cfg(feature = "teste-irq-fifo-cheia")]
    pub fn vazia_armada(&self, referencia: u64) -> bool {
        self.referencia == referencia && !self.pendente && self.entregue == 0
    }
    pub fn retirar(&mut self) -> Result<Option<u64>, u64> {
        if !self.pendente || self.entregue != 0 {
            return Ok(None);
        }
        if self.proxima == 0 {
            return Err(erro::RECURSOS);
        }
        self.pendente = false;
        let sequencia = self.proxima;
        self.proxima = self.proxima.checked_add(1).unwrap_or(0);
        self.entregue = sequencia;
        Ok(Some(sequencia))
    }
    pub fn confirmar(&mut self, referencia: u64, sequencia: u64) -> Result<(), u64> {
        if self.referencia != referencia || sequencia == 0 || self.entregue != sequencia {
            return Err(erro::ESTADO);
        }
        self.entregue = 0;
        Ok(())
    }
    pub fn cancelar(&mut self) {
        self.referencia = 0;
        self.pendente = false;
        self.entregue = 0;
    }
    /// Observação read-only para o ensaio de revogação; chamador mantém IF=0.
    #[cfg(feature = "teste-irq-morte")]
    pub fn sequencia_entregue(&self) -> u64 {
        self.entregue
    }
    /// A consulta é somente leitura e deve ocorrer com IF=0.
    #[cfg(feature = "teste-irq-morte")]
    pub fn ocupada(&self, referencia: u64) -> bool {
        self.referencia == referencia && (self.pendente || self.entregue != 0)
    }
}
#[cfg(test)]
mod testes {
    use super::*;
    use crate::agendamento::{expirar_recepcao, EsperaIpc, Estado};
    use crate::controle::{retirar_atual, CaixaMensagens};
    use contrato_sistema::{criar_referencia, Mensagem, REDE, SUPERVISOR, TERMINAL};

    fn referencia(id: u64, geracao: u64) -> u64 {
        criar_referencia(id, geracao).unwrap()
    }

    fn mensagem(origem: u64, sequencia: u64) -> Mensagem {
        Mensagem {
            origem,
            codigo: 0x55,
            a: sequencia,
            ..Mensagem::default()
        }
    }

    #[test]
    fn coalescencia_autoridade_e_sequencia() {
        let mut i = Irq::NOVA;
        assert_eq!(i.armar(7), Ok(()));
        #[cfg(feature = "teste-seletor-irq-ipc")]
        assert!(!i.selecionavel(7));
        assert_eq!(i.armar(7), Err(erro::ESTADO));
        i.publicar();
        #[cfg(feature = "teste-seletor-irq-ipc")]
        assert!(i.selecionavel(7));
        i.publicar();
        let s = i.retirar().unwrap().unwrap();
        #[cfg(feature = "teste-seletor-irq-ipc")]
        assert!(!i.selecionavel(7));
        assert_eq!(i.retirar(), Ok(None));
        assert_eq!(i.confirmar(8, s), Err(erro::ESTADO));
        assert_eq!(i.confirmar(7, s + 1), Err(erro::ESTADO));
        assert_eq!(i.confirmar(7, s), Ok(()));
        assert_eq!(i.confirmar(7, s), Err(erro::ESTADO));
        i.publicar();
        assert_ne!(i.retirar(), Ok(Some(s)));
        i.cancelar();
        assert_eq!(i.confirmar(7, s), Err(erro::ESTADO));
    }
    #[test]
    fn exaustao_nao_reutiliza_sequencia() {
        let mut i = Irq::NOVA;
        i.armar(7).unwrap();
        i.proxima = u64::MAX;
        i.publicar();
        assert_eq!(i.retirar(), Ok(Some(u64::MAX)));
        i.confirmar(7, u64::MAX).unwrap();
        i.publicar();
        assert_eq!(i.retirar(), Err(erro::RECURSOS));
    }

    #[test]
    fn irq_coalescida_permanece_separada_de_ipc_quando_a_fila_esta_cheia() {
        let terminal = referencia(TERMINAL, 1);
        let atuais = [terminal, referencia(REDE, 1), referencia(SUPERVISOR, 1)];
        let mut fila = CaixaMensagens::NOVA;
        for n in 0..crate::controle::CAPACIDADE {
            fila.inserir(mensagem(terminal, n as u64)).unwrap();
        }
        assert_eq!(fila.inserir(mensagem(terminal, 99)), Err(erro::FILA_CHEIA));

        let mut irq = Irq::NOVA;
        irq.armar(terminal).unwrap();
        for _ in 0..8 {
            irq.publicar();
        }
        let sequencia = irq.retirar().unwrap().unwrap();
        assert_eq!(irq.retirar(), Ok(None));
        assert_eq!(fila.tamanho(), crate::controle::CAPACIDADE);

        irq.confirmar(terminal, sequencia).unwrap();
        assert_eq!(retirar_atual(&mut fila, &atuais).unwrap().a, 0);
        assert_eq!(fila.tamanho(), crate::controle::CAPACIDADE - 1);
        irq.publicar();
        assert_eq!(irq.retirar(), Ok(Some(sequencia + 1)));
    }

    #[test]
    fn prazo_vencido_precede_irq_e_ipc_sem_consumir_nenhuma_fonte() {
        let terminal = referencia(TERMINAL, 1);
        let atuais = [terminal, referencia(REDE, 1), referencia(SUPERVISOR, 1)];
        let mut fila = CaixaMensagens::NOVA;
        fila.inserir(mensagem(terminal, 42)).unwrap();

        let mut irq = Irq::NOVA;
        irq.armar(terminal).unwrap();
        irq.publicar();

        let mut estado = Estado::ATIVO;
        let mut espera = EsperaIpc::NOVA;
        assert_eq!(espera.registrar(&mut estado, 0), Ok(true));
        let mut recepcao_pendente = true;
        let mut prazo = Some(10);

        assert!(expirar_recepcao(
            10,
            &mut prazo,
            &mut recepcao_pendente,
            &mut espera,
            &mut estado
        ));
        assert!(estado.executavel());
        assert!(!recepcao_pendente);
        assert_eq!(prazo, None);

        // O timeout conclui a espera, mas deixa ambos os eventos para uma
        // consulta posterior, como exige a precedência definida para a ABI.
        assert_eq!(irq.retirar().unwrap(), Some(1));
        assert_eq!(retirar_atual(&mut fila, &atuais).unwrap().a, 42);
        assert_eq!(fila.tamanho(), 0);
    }

    #[test]
    fn revogacao_rejeita_sequencia_irq_antiga_e_filtra_ipc_por_geracao() {
        let terminal_antigo = referencia(TERMINAL, 1);
        let terminal_atual = referencia(TERMINAL, 2);
        let rede_antiga = referencia(REDE, 1);
        let rede_atual = referencia(REDE, 2);
        let atuais = [terminal_atual, rede_atual, referencia(SUPERVISOR, 1)];
        let mut fila = CaixaMensagens::NOVA;
        fila.inserir(mensagem(rede_antiga, 1)).unwrap();
        fila.inserir(mensagem(rede_atual, 2)).unwrap();

        let mut irq = Irq::NOVA;
        irq.armar(terminal_antigo).unwrap();
        irq.publicar();
        let sequencia_antiga = irq.retirar().unwrap().unwrap();
        irq.cancelar();
        irq.armar(terminal_atual).unwrap();

        assert_eq!(
            irq.confirmar(terminal_antigo, sequencia_antiga),
            Err(erro::ESTADO)
        );
        assert_eq!(
            irq.confirmar(terminal_atual, sequencia_antiga),
            Err(erro::ESTADO)
        );
        assert_eq!(irq.retirar(), Ok(None));
        assert_eq!(
            retirar_atual(&mut fila, &atuais).unwrap().a,
            2,
            "a mensagem da geração revogada não pode preceder nem substituir a atual"
        );
        assert_eq!(fila.tamanho(), 0);

        irq.publicar();
        let sequencia_atual = irq.retirar().unwrap().unwrap();
        assert_eq!(sequencia_atual, sequencia_antiga + 1);
        assert_eq!(
            irq.confirmar(terminal_atual, sequencia_antiga),
            Err(erro::ESTADO)
        );
        irq.confirmar(terminal_atual, sequencia_atual).unwrap();
    }
}
