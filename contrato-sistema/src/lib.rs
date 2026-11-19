#![no_std]
//! ABI v0.2 do microkernel. Referencias identificam uma encarnacao de servico.
//! Mensagens de tamanho fixo; nenhum ponteiro de usuario cruza a fronteira IPC.
pub use contrato_boot::Tela;

pub const ASSINATURA_SERVICO: u64 = 0x5241_495a_5352_5631;
pub const VERSAO_ABI: u64 = 2;
pub const BASE_USUARIO: u64 = 0x4000_0000;
pub const TAMANHO_USUARIO: usize = 2 * 1024 * 1024;
pub const INICIO_PILHA: u64 = 0x401e_0000;
pub const TOPO_PILHA: u64 = 0x401f_e000;
pub const INFO_USUARIO: u64 = 0x401f_f000;
pub const MMIO_REDE: u64 = 0x5000_0000;
pub const FRAMEBUFFER_USUARIO: u64 = 0x6000_0000;
pub const TERMINAL: u64 = 1;
pub const REDE: u64 = 2;
pub const SUPERVISOR: u64 = 3;
pub const QUANTIDADE_SERVICOS: usize = 3;
/// Limites estaticos da caixa IPC e de cada origem na caixa do terminal.
pub const CAPACIDADE_CAIXA_IPC: usize = 16;
pub const LIMITE_ORIGEM_CAIXA_TERMINAL: usize = 15;
/// Inicializacao original e mais tres reinicios; as arenas DMA nao sao reutilizadas.
pub const MAX_INSTANCIAS_REDE: u64 = 4;

/// Identidade de instancia, nao uma capability: o kernel verifica direitos separados.
/// Bits 63..32: geracao nao nula. Bits 31..0: identidade logica do servico.
pub const fn criar_referencia(id: u64, geracao: u64) -> Option<u64> {
    if id == 0 || id > QUANTIDADE_SERVICOS as u64 || geracao == 0 || geracao > u32::MAX as u64 {
        None
    } else {
        Some((geracao << 32) | id)
    }
}
pub const fn id_referencia(referencia: u64) -> u64 {
    referencia & u32::MAX as u64
}
pub const fn geracao_referencia(referencia: u64) -> u64 {
    referencia >> 32
}
pub const fn referencia_valida(referencia: u64) -> bool {
    criar_referencia(id_referencia(referencia), geracao_referencia(referencia)).is_some()
}

pub mod estado {
    pub const INEXISTENTE: u64 = 0;
    pub const EXECUTAVEL: u64 = 1;
    pub const FALHOU: u64 = 2;
    pub const FINALIZADO: u64 = 3;
}

pub mod chamada {
    pub const CEDER: u64 = 0;
    pub const ENVIAR: u64 = 1;
    pub const RECEBER: u64 = 2;
    pub const SAIR: u64 = 3;
    pub const PCI_LER: u64 = 4;
    pub const PCI_COMANDO: u64 = 5;
    pub const ESTADO_SERVICO: u64 = 6;
    pub const TIQUES: u64 = 7;
    /// rdi=id logico; retorna referencia atual mesmo se falhou/finalizou.
    pub const RESOLVER: u64 = 8;
    /// rdi=referencia exata da rede; exclusivo do supervisor; retorna nova referencia.
    pub const REINICIAR: u64 = 9;
    /// rdi=referencia exata da rede; exclusivo do supervisor; retorna zero.
    pub const ENCERRAR: u64 = 10;
    /// Espera disponibilidade da caixa propria, sem consumir mensagem.
    pub const ESPERAR_IPC: u64 = 11;
    /// Recebe uma mensagem da propria caixa, bloqueando quando vazia.
    pub const RECEBER_BLOQUEANTE: u64 = 12;
    /// Recebe da caixa propria com modo e duracao explicitos.
    pub const RECEBER_COM_PRAZO: u64 = 13;
    /// Recebe IPC proprio ou informa indisponibilidade da rede observada.
    pub const RECEBER_OBSERVANDO: u64 = 14;
    pub const INSCREVER_VIDA: u64 = 15;
    pub const CANCELAR_VIDA: u64 = 16;
    pub const ESPERAR_EVENTO: u64 = 17;
    pub const ARMAR_IRQ4: u64 = 18;
    pub const CONFIRMAR_IRQ4: u64 = 19;
    pub const ESPERAR_IRQ_IPC: u64 = 20;
}
pub mod evento_vida {
    pub const ESTADO: u64 = 1;
    pub const INVALIDACAO: u64 = 2;
}
pub mod classe_evento {
    pub const IPC: u64 = 1;
    pub const VIDA: u64 = 2;
    pub const IRQ: u64 = 3;
}
pub mod prazo {
    pub const IMEDIATO: u64 = 0;
    pub const FINITO: u64 = 1;
    pub const SEM_PRAZO: u64 = 2;
}
pub mod erro {
    pub const PERMISSAO: u64 = u64::MAX - 1;
    pub const FILA_CHEIA: u64 = u64::MAX - 2;
    pub const SERVICO: u64 = u64::MAX - 3;
    pub const CHAMADA: u64 = u64::MAX - 4;
    pub const REFERENCIA_ANTIGA: u64 = u64::MAX - 5;
    pub const RECURSOS: u64 = u64::MAX - 6;
    pub const ESTADO: u64 = u64::MAX - 7;
    pub const IMAGEM: u64 = u64::MAX - 8;
    pub const TEMPO: u64 = u64::MAX - 9;
    pub const AUSENTE: u64 = u64::MAX - 10;
    pub const ALVO_INDISPONIVEL: u64 = u64::MAX - 11;
}
pub mod evento {
    pub const REDE_PRONTA: u64 = 1; // a=0 ok ou 1 erro; b=MAC; c=IPv4 big endian; d=CPL
    pub const PING_SOLICITAR: u64 = 2; // a=IPv4 big endian
    pub const PING_OK: u64 = 3; // a=IPv4; b=sequencia
    pub const REDE_STATUS_SOLICITAR: u64 = 4;
    pub const REDE_STATUS: u64 = 5; // a=RX; b=TX; c=ping_ok; d=udp_eco
    pub const REDE_FALHAR: u64 = 6; // teste UD2 no processo de rede
    pub const REDE_ERRO: u64 = 7; // a=codigo interno
    pub const REDE_OCUPAR: u64 = 8; // teste de preempcao: loop sem ceder
    pub const REDE_ACESSAR_NUCLEO: u64 = 9; // leitura proibida para testar MMU
    pub const REDE_EM_LACO: u64 = 10; // aviso enviado imediatamente antes do loop de teste
    pub const SUPERVISOR_PRONTO: u64 = 11; // a=modo; b=limite encarnacoes; c=CPL
    pub const SUPERVISAO_CONFIGURAR: u64 = 12; // a=0 manual ou 1 automatico
    pub const SUPERVISAO_ESTADO: u64 = 13; // a=modo; b=reinicios; c=ultimo erro; d=limite
    pub const SERVICO_REINICIAR: u64 = 14; // a=referencia exata a substituir
    pub const SERVICO_REINICIADO: u64 = 15; // a=antiga; b=nova; c=1 automatico ou 0 manual
    pub const SERVICO_ENCERRAR: u64 = 16; // a=referencia exata a encerrar
    pub const SERVICO_ENCERRADO: u64 = 17; // a=referencia encerrada
    pub const SUPERVISOR_ERRO: u64 = 18; // a=erro; b=operacao; c=referencia relacionada
    pub const SUPERVISAO_CONSULTAR: u64 = 19;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Mensagem {
    /// Referencia completa do remetente, preenchida pelo kernel.
    pub origem: u64,
    pub codigo: u64,
    pub a: u64,
    pub b: u64,
    pub c: u64,
    pub d: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct InfoServico {
    pub assinatura: u64,
    pub versao: u64,
    pub id: u64,
    /// Identidade logica e geracao desta instancia. ABI 2; nao compativel com ABI 1.
    pub referencia: u64,
    /// Endereco fisico da reserva de 2 MiB: somente driver atribuido usa para DMA.
    pub base_fisica: u64,
    pub tamanho_imagem: u64,
    pub tela: Tela,
    pub memoria_livre_boot: u64,
    pub nucleo_inicio: u64,
    pub nucleo_fim: u64,
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn referencias_preservam_identidade_e_geracao_sem_colisoes() {
        for id in 1..=QUANTIDADE_SERVICOS as u64 {
            for geracao in [1, 2, MAX_INSTANCIAS_REDE, u32::MAX as u64] {
                let r = criar_referencia(id, geracao).unwrap();
                assert_eq!(id_referencia(r), id);
                assert_eq!(geracao_referencia(r), geracao);
                assert!(referencia_valida(r));
                assert_ne!(r, id);
                assert!(r < erro::IMAGEM);
            }
        }
        assert_ne!(criar_referencia(REDE, 1), criar_referencia(REDE, 2));
        assert_ne!(criar_referencia(REDE, 1), criar_referencia(TERMINAL, 1));
    }

    #[test]
    fn referencias_invalidas_e_codigos_de_erro_sao_recusados() {
        for (id, geracao) in [(0, 1), (4, 1), (REDE, 0), (REDE, u32::MAX as u64 + 1)] {
            assert_eq!(criar_referencia(id, geracao), None);
        }
        for r in [
            0,
            TERMINAL,
            REDE,
            SUPERVISOR,
            erro::PERMISSAO,
            erro::RECURSOS,
        ] {
            assert!(!referencia_valida(r));
        }
    }

    #[test]
    fn layout_binario_da_abi_dois_e_estavel() {
        assert_eq!(core::mem::size_of::<Mensagem>(), 48);
        assert_eq!(core::mem::size_of::<InfoServico>(), 104);
        assert_eq!(core::mem::align_of::<InfoServico>(), 8);
        assert_eq!(core::mem::offset_of!(InfoServico, id), 16);
        assert_eq!(core::mem::offset_of!(InfoServico, referencia), 24);
        assert_eq!(core::mem::offset_of!(InfoServico, base_fisica), 32);
        assert_eq!(core::mem::offset_of!(InfoServico, tela), 48);
    }
}
