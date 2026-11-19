#![no_std]
#![no_main]

mod console;
mod fonte;
mod io;
mod teclado;

use apoio_servicos as sistema;
use contrato_sistema::{
    chamada, erro, evento, geracao_referencia, id_referencia, referencia_valida, InfoServico,
    Mensagem, ASSINATURA_SERVICO, REDE, SUPERVISOR, TERMINAL, VERSAO_ABI,
};

struct ContextoTerminal {
    referencia_guardada: Option<u64>,
}

#[cfg(feature = "teste-ciclo-espera")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SnapshotDriverCiclo {
    referencia: u64,
    mmio: u64,
    arena_fisica: u64,
    indice_rx: u64,
    indice_tx: u64,
    recebidos: u64,
    transmitidos: u64,
}

#[cfg(feature = "teste-ciclo-espera")]
#[derive(Clone, Copy)]
struct SnapshotDriverCicloPendente {
    codigo_inicio: u64,
    referencia: u64,
    mmio: u64,
    arena_fisica: u64,
    indice_rx: u64,
}

#[cfg(feature = "teste-rx-quota")]
struct TesteRxQuota {
    etapa: u8,
    inicio: u64,
    fifo_rede_pronta: bool,
    recebidos_rede: u64,
    controle_visto: bool,
}

#[cfg(feature = "teste-rx-quota")]
impl TesteRxQuota {
    const NOVO: Self = Self {
        etapa: 0,
        inicio: 0,
        fifo_rede_pronta: false,
        recebidos_rede: 0,
        controle_visto: false,
    };

    fn falhar(&mut self, motivo: &str) {
        imprimirln!("\n[teste-rx-quota] FALHA motivo={}", motivo);
        self.etapa = 9;
    }

    fn tratar_ipc(&mut self, mensagem: &Mensagem) {
        let rede = sistema::resolver(REDE);
        let supervisor = sistema::resolver(SUPERVISOR);
        match (self.etapa, mensagem.codigo) {
            (3, 0x7ba) if mensagem.origem == rede && self.recebidos_rede < 15 => {
                let esperado = self.recebidos_rede + 1;
                if mensagem.a != esperado
                    || mensagem.b != 0x525a32355154
                    || mensagem.c != 0
                    || mensagem.d != 0
                {
                    self.falhar("quadro_fifo_origem_id_ou_payload");
                    return;
                }
                self.recebidos_rede = esperado;
            }
            (3, 0x7be)
                if mensagem.origem == supervisor
                    && self.recebidos_rede == 15
                    && !self.controle_visto =>
            {
                if mensagem.a != 1 || mensagem.b != 0x44 || mensagem.c != 0x55 || mensagem.d != 0x66
                {
                    self.falhar("controle_supervisor_payload");
                    return;
                }
                self.controle_visto = true;
                if sistema::enviar(supervisor, 0x7c5, 0, 0, 0, 0) != 0 {
                    self.falhar("consulta_resultado_supervisor");
                    return;
                }
                self.etapa = 4;
            }
            (4, 0x7c2) if mensagem.origem == supervisor => {
                if mensagem.a != 0
                    || mensagem.b != erro::FILA_CHEIA
                    || mensagem.c != 3
                    || mensagem.d != 1
                {
                    self.falhar("resultado_controle_supervisor");
                    return;
                }
                if sistema::enviar(rede, 0x7bf, 16, 0x525a32355154, 0, 0) != 0 {
                    self.falhar("liberacao_retry_rede");
                    return;
                }
                self.etapa = 5;
            }
            (5, 0x7ba) if mensagem.origem == rede => {
                if mensagem.a != 16
                    || mensagem.b != 0x525a32355154
                    || mensagem.c != 0
                    || mensagem.d != 0
                {
                    self.falhar("retry_id16");
                    return;
                }
                self.recebidos_rede = 16;
                if sistema::enviar(rede, 0x7c4, 0, 0, 0, 0) != 0 {
                    self.falhar("consulta_driver_lote_inicial");
                    return;
                }
                self.etapa = 6;
            }
            (6, 0x7c3) if mensagem.origem == rede => {
                if mensagem.a != 16 || mensagem.b != 0xffff || mensagem.c != 1 || mensagem.d != 0 {
                    self.falhar("snapshot_driver_16");
                    return;
                }
                if sistema::enviar(rede, 0x7c8, 16, 0x525a32355154, 0, 0) != 0 {
                    self.falhar("liberacao_continuacao_rx");
                    return;
                }
                imprimirln!(
                    "\n[teste-rx-quota] RETRY_OK id=16 quadros_driver=16 mascara=0xffff retries=1 driver_invalido=0"
                );
                imprimirln!("[teste-rx-quota] CONTINUACAO_PRONTA ids=17..18");
                self.etapa = 7;
            }
            (7, 0x7ba) if mensagem.origem == rede && self.recebidos_rede < 18 => {
                let esperado = self.recebidos_rede + 1;
                if mensagem.a != esperado
                    || !matches!(esperado, 17 | 18)
                    || mensagem.b != 0x525a32355154
                    || mensagem.c != 0
                    || mensagem.d != 0
                {
                    self.falhar("continuacao_rx_id_ou_payload");
                    return;
                }
                self.recebidos_rede = esperado;
                if self.recebidos_rede == 18 {
                    if sistema::enviar(rede, 0x7c4, 0, 0, 0, 0) != 0 {
                        self.falhar("consulta_driver_final");
                        return;
                    }
                    self.etapa = 8;
                }
            }
            (8, 0x7c3) if mensagem.origem == rede => {
                if mensagem.a != 18 || mensagem.b != 0x3ffff || mensagem.c != 1 || mensagem.d != 0 {
                    self.falhar("snapshot_driver_final");
                    return;
                }
                imprimirln!("\n[teste-rx-quota] RESULTADO rx=18 ipc_rede=18 controle_supervisor=1 fila_cheia=1 retry_id=16 ordem=1..18 driver_invalido=0 CPL3");
                self.etapa = 10;
            }
            (_, 0x7c6) => self.falhar("servico_rejeitou_fixture"),
            (etapa, codigo)
                if (1..=8).contains(&etapa)
                    && matches!(codigo, 0x7ba | 0x7be | 0x7c2 | 0x7c3 | 0x7c6) =>
            {
                self.falhar("ipc_fora_de_fase_ou_origem");
            }
            _ => {}
        }
    }
}

#[cfg(feature = "comparacao-consultas-vazias")]
const JANELA_COMPARACAO_TIQUES: u64 = 100;

// Contagem de pontos observados no loop, não mede CPU nem custo de execução.
#[cfg(feature = "comparacao-consultas-vazias")]
struct MedicaoConsultasVazias {
    ativa: bool,
    concluida: bool,
    inicio: u64,
    consultas_ipc_vazias: u64,
    voltas_sem_trabalho: u64,
}

#[cfg(feature = "comparacao-consultas-vazias")]
impl MedicaoConsultasVazias {
    const fn nova() -> Self {
        Self {
            ativa: false,
            concluida: false,
            inicio: 0,
            consultas_ipc_vazias: 0,
            voltas_sem_trabalho: 0,
        }
    }

    fn armar(&mut self) {
        if self.ativa || self.concluida {
            imprimirln!("[comparacao-vazia] FALHA_ESTADO");
            return;
        }
        self.inicio = sistema::tiques();
        self.consultas_ipc_vazias = 0;
        self.voltas_sem_trabalho = 0;
        self.ativa = true;
        imprimirln!(
            "\n[comparacao-vazia] ARMADA revisao=RZ-025 janela_tiques={} inicio={}",
            JANELA_COMPARACAO_TIQUES,
            self.inicio
        );
    }

    fn concluir_se_vencida(&mut self) {
        if !self.ativa {
            return;
        }
        let fim = sistema::tiques();
        let decorrido = fim.wrapping_sub(self.inicio);
        if decorrido < JANELA_COMPARACAO_TIQUES {
            return;
        }
        self.ativa = false;
        self.concluida = true;
        if decorrido != JANELA_COMPARACAO_TIQUES {
            imprimirln!(
                "\n[comparacao-vazia] FALHA_JANELA inicio={} fim={} decorrido={} esperado={}",
                self.inicio,
                fim,
                decorrido,
                JANELA_COMPARACAO_TIQUES
            );
            return;
        }
        imprimirln!(
            "\n[comparacao-vazia] RESULTADO revisao=RZ-025 janela_tiques={} inicio={} fim={} decorrido={} consultas_ipc_vazias={} voltas_sem_trabalho={}",
            JANELA_COMPARACAO_TIQUES,
            self.inicio,
            fim,
            decorrido,
            self.consultas_ipc_vazias,
            self.voltas_sem_trabalho
        );
    }
}

#[cfg(feature = "teste-rz024")]
fn armar_janela_idle_rz024(cenario: u64) -> u64 {
    let retorno;
    unsafe {
        // Chamada privada apenas nas imagens opt-in. O kernel valida terminal
        // CPL3, cenário e uso único; não altera ABI pública nem espera.
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") 0xff04u64 => retorno,
            in("rdi") cenario,
            lateout("rsi") _, lateout("rdx") _, lateout("r10") _,
            lateout("r8") _, lateout("r9") _,
        );
    }
    retorno
}

#[cfg(feature = "teste-seletor-irq-ipc")]
fn sondar_irq_ipc_teste() -> (u64, u64, u64, u64, u64) {
    let (estado, mensagens, preferir_irq, irq_pendente, irq_mascarada);
    unsafe {
        // Esta interrupcao privada so existe na imagem opt-in. O kernel exige
        // terminal CPL3 e apenas observa o slot, a FIFO e o IMR, sem consumi-los.
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") 0xff01u64 => estado,
            inlateout("rdi") 0x7e9u64 => mensagens,
            inlateout("rsi") 1u64 => preferir_irq,
            inlateout("rdx") 2u64 => irq_pendente,
            lateout("r10") irq_mascarada,
        );
    }
    (estado, mensagens, preferir_irq, irq_pendente, irq_mascarada)
}

#[cfg(feature = "teste-irq-fifo-cheia")]
fn sondar_fifo_irq_teste(modo: u64) -> (u64, u64, u64, u64, u64) {
    let (estado, quantidade, preferir_irq, irq_estado, mascara);
    unsafe {
        // Sonda privada somente leitura, autorizada pelo kernel para terminal
        // CPL3. A syscall roda com IF=0; FIFO/slot/IMR são um único snapshot.
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") 0xff05u64 => estado,
            inlateout("rdi") modo => _,
            lateout("rsi") quantidade,
            lateout("rdx") preferir_irq,
            lateout("r10") irq_estado,
            lateout("r8") mascara,
        );
    }
    (estado, quantidade, preferir_irq, irq_estado, mascara)
}

#[cfg(feature = "teste-irq-fifo-cheia")]
fn testar_irq_fifo_cheia() {
    fn falha(motivo: &str) -> ! {
        imprimirln!("[teste-irq-fifo-cheia] FALHA_{}", motivo);
        sistema::sair()
    }
    if sistema::cpl() != 3 || sistema::enviar(sistema::resolver(REDE), 0x7d8, 15, 0, 0, 0) != 0 {
        falha("PEDIDO_REDE");
    }
    let inicio = sistema::tiques();
    loop {
        let (estado, quantidade, _, irq_vazia, mascara) = sondar_fifo_irq_teste(1);
        if estado == erro::PERMISSAO {
            falha("SONDA_PERMISSAO");
        }
        if estado == 1 {
            if quantidade != 15 || irq_vazia != 1 || mascara != 0 {
                falha("PRECONDICAO_REDE");
            }
            imprimirln!(
                "\n[teste-irq-fifo-cheia] REDE_FIFO quantidade={} origem={:#x} ordem=1..15 irq_vazia=1",
                quantidade,
                sistema::resolver(REDE)
            );
            break;
        }
        if sistema::tiques().wrapping_sub(inicio) >= 1000 {
            imprimirln!("[teste-irq-fifo-cheia] TIMEOUT_REDE");
            falha("TIMEOUT_REDE");
        }
        sistema::ceder();
    }
    if sistema::enviar(sistema::resolver(SUPERVISOR), 0x7d9, 1, 0, 0, 0) != 0 {
        falha("PEDIDO_SUPERVISOR");
    }
    let inicio = sistema::tiques();
    loop {
        let (estado, quantidade, _, irq_vazia, mascara) = sondar_fifo_irq_teste(2);
        if estado == erro::PERMISSAO {
            falha("SONDA_PERMISSAO");
        }
        if estado == 1 {
            if quantidade != 16 || irq_vazia != 1 || mascara != 0 {
                falha("PRECONDICAO_FIFO_CHEIA");
            }
            imprimirln!(
                "\n[teste-irq-fifo-cheia] FIFO_CHEIA quantidade=16 rede=15 supervisor=1 irq_vazia=1 imr_irq4=0 CPL3"
            );
            break;
        }
        if sistema::tiques().wrapping_sub(inicio) >= 1000 {
            imprimirln!("[teste-irq-fifo-cheia] TIMEOUT_SUPERVISOR");
            falha("TIMEOUT_SUPERVISOR");
        }
        sistema::ceder();
    }

    let inicio = sistema::tiques();
    loop {
        let (estado, quantidade, _, irq_pendente, mascara) = sondar_fifo_irq_teste(3);
        if estado == erro::PERMISSAO {
            falha("SONDA_PERMISSAO");
        }
        if estado == 1 {
            if quantidade != 16 || irq_pendente != 1 || mascara != 1 {
                falha("PRECONDICAO_IRQ");
            }
            imprimirln!(
                "\n[teste-irq-fifo-cheia] IRQ_PENDENTE fifo=16 irq_selecionavel=1 imr_irq4=1"
            );
            break;
        }
        if sistema::tiques().wrapping_sub(inicio) >= 1000 {
            imprimirln!("[teste-irq-fifo-cheia] TIMEOUT_IRQ");
            falha("TIMEOUT_IRQ");
        }
        sistema::ceder();
    }
    let inicio_byte = sistema::tiques();
    let byte = loop {
        if let Some(byte) = io::ler_serial() {
            break byte;
        }
        if sistema::tiques().wrapping_sub(inicio_byte) >= 1000 {
            falha("TIMEOUT_BYTE");
        }
        sistema::ceder();
    };
    if byte != b'A' {
        falha("BYTE_UART");
    }
    imprimirln!("[teste-irq-fifo-cheia] BYTE_UART valor=0x41");

    let mut proximo_rede = 1u64;
    let mut supervisor_recebido = false;
    let mut irq_seq = 0u64;
    let inicio_dreno = sistema::tiques();
    while proximo_rede <= 15 || !supervisor_recebido || irq_seq == 0 {
        match sistema::esperar_irq_ipc(contrato_sistema::prazo::IMEDIATO, 0) {
            Ok(sistema::EventoSistema::Ipc(m)) if m.origem == sistema::resolver(REDE) => {
                if supervisor_recebido
                    || m.codigo != 0x7da
                    || m.a != proximo_rede
                    || m.b != 0x11
                    || m.c != 0x22
                    || m.d != 0x33
                {
                    falha("ORDEM_REDE");
                }
                proximo_rede += 1;
            }
            Ok(sistema::EventoSistema::Ipc(m)) if m.origem == sistema::resolver(SUPERVISOR) => {
                if supervisor_recebido
                    || proximo_rede != 16
                    || m.codigo != 0x7db
                    || m.a != 1
                    || m.b != 0x44
                    || m.c != 0x55
                    || m.d != 0x66
                {
                    falha("ORDEM_SUPERVISOR");
                }
                supervisor_recebido = true;
            }
            Ok(sistema::EventoSistema::Irq { sequencia }) if irq_seq == 0 && sequencia != 0 => {
                irq_seq = sequencia;
                if sistema::confirmar_irq4(sequencia) != 0 {
                    falha("CONFIRMACAO_IRQ");
                }
                imprimirln!("[teste-irq-fifo-cheia] IRQ_ENTREGUE seq={}", sequencia);
            }
            _ => falha("EVENTO_INESPERADO"),
        }
        if sistema::tiques().wrapping_sub(inicio_dreno) >= 1000 {
            falha("TIMEOUT_DRENO");
        }
    }
    if proximo_rede != 16 || !supervisor_recebido || irq_seq == 0 {
        falha("CONTAGEM_FINAL");
    }
    let (estado, quantidade, _, irq_vazia, mascara) = sondar_fifo_irq_teste(4);
    if estado != 1 || quantidade != 0 || irq_vazia != 1 || mascara != 0 {
        falha("FILA_NAO_VAZIA");
    }
    imprimirln!(
        "[teste-irq-fifo-cheia] DRENO_OK rede=15 supervisor=1 fifo=0 irq_seq={} byte=0x41",
        irq_seq
    );
    imprimirln!("[teste-irq-fifo-cheia] FIM CPL3");
}

#[cfg(feature = "teste-seletor-irq-ipc")]
fn testar_irq_ipc() {
    fn falha(motivo: &str) -> ! {
        imprimirln!("[teste-seletor-irq-ipc] FALHA_{}", motivo);
        sistema::sair()
    }
    if sistema::cpl() != 3 {
        falha("CPL");
    }
    if sistema::enviar(sistema::resolver(REDE), 0x7e8, 0x21, 0, 0, 0) != 0 {
        falha("PEDIDO_IPC");
    }
    imprimirln!("\n[teste-seletor-irq-ipc] CARGA_PRONTA CPL3");
    let inicio = sistema::tiques();
    let mut irq_pendente_anunciada = false;
    let mut byte_a_lido = false;
    loop {
        let (estado, mensagens, preferir_irq, irq_pendente, irq_mascarada) = sondar_irq_ipc_teste();
        if estado == erro::PERMISSAO {
            falha("SONDA_PERMISSAO");
        }
        if estado > 1 {
            falha("SONDA_RETORNO");
        }
        // Nao drena o RBR ate o kernel confirmar que a IRQ4 real foi publicada.
        if irq_pendente == 1 && !irq_pendente_anunciada {
            if irq_mascarada != 1 {
                falha("IRQ_NAO_MASCARADA");
            }
            irq_pendente_anunciada = true;
            imprimirln!("[teste-seletor-irq-ipc] IRQ4_PENDENTE mascara=1");
        }
        if irq_pendente == 1 && !byte_a_lido {
            if let Some(byte) = io::ler_serial() {
                if byte != b'A' {
                    falha("BYTE_UART");
                }
                byte_a_lido = true;
                imprimirln!("[teste-seletor-irq-ipc] BYTE_A_LIDO");
            }
        }
        if estado == 1 && byte_a_lido {
            if mensagens != 2 || preferir_irq != 0 || irq_pendente != 1 || irq_mascarada != 1 {
                falha("PRECONDICAO");
            }
            imprimirln!(
                "[teste-seletor-irq-ipc] AMBAS_PENDENTES irq=1 ipc={} mascara=1 preferir_irq=0",
                mensagens
            );
            break;
        }
        if sistema::tiques().wrapping_sub(inicio) >= 1000 {
            imprimirln!(
                "[teste-seletor-irq-ipc] DIAGNOSTICO_SONDA estado={} ipc_validas={} preferir_irq={} irq_pendente={} mascara={} byte_a={}",
                estado,
                mensagens,
                preferir_irq,
                irq_pendente,
                irq_mascarada,
                byte_a_lido as u8
            );
            falha("TIMEOUT_SONDA");
        }
        sistema::ceder();
    }

    match sistema::esperar_irq_ipc(contrato_sistema::prazo::IMEDIATO, 0) {
        Ok(sistema::EventoSistema::Ipc(m))
            if m.origem == sistema::resolver(REDE)
                && m.codigo == 0x7e9
                && m.a == 1
                && m.b == 0
                && m.c == 0
                && m.d == 0 =>
        {
            imprimirln!(
                "[teste-seletor-irq-ipc] PRIMEIRA_IPC origem={:#x} codigo={:#x} id={}",
                m.origem,
                m.codigo,
                m.a
            );
        }
        _ => falha("ORDEM_IPC"),
    }
    let sequencia = match sistema::esperar_irq_ipc(contrato_sistema::prazo::IMEDIATO, 0) {
        Ok(sistema::EventoSistema::Irq { sequencia }) if sequencia != 0 => sequencia,
        _ => falha("ORDEM_IRQ"),
    };
    if sistema::confirmar_irq4(sequencia) != 0 {
        falha("CONFIRMACAO");
    }
    imprimirln!("[teste-seletor-irq-ipc] IRQ4_CONFIRMADA seq={}", sequencia);
    match sistema::receber_com_prazo(contrato_sistema::prazo::IMEDIATO, 0) {
        Ok(m)
            if m.origem == sistema::resolver(REDE)
                && m.codigo == 0x7e9
                && m.a == 2
                && m.b == 0
                && m.c == 0
                && m.d == 0 =>
        {
            imprimirln!(
                "[teste-seletor-irq-ipc] IPC_RESTANTE origem={:#x} codigo={:#x} id={}",
                m.origem,
                m.codigo,
                m.a
            );
        }
        _ => falha("IPC_RESTANTE"),
    }
    imprimirln!("[teste-seletor-irq-ipc] FIM CPL3");
}

#[cfg(feature = "teste-irq")]
fn testar_irq() {
    fn falha() {
        imprimirln!("[teste-irq] FALHA");
    }
    // Instrumentacao exclusiva da imagem de ensaio; nao integra a ABI.
    fn amostrar_irq4() -> (u64, u64) {
        let (contador, mascara);
        unsafe {
            core::arch::asm!("int 0x80", inlateout("rax") 0xff00u64 => contador,
                lateout("rdi") mascara, lateout("rsi") _, lateout("rdx") _,
                lateout("r10") _, lateout("r8") _, lateout("r9") _);
        }
        (contador, mascara)
    }
    fn irq() -> Option<u64> {
        match sistema::esperar_irq_ipc(contrato_sistema::prazo::SEM_PRAZO, 0) {
            Ok(sistema::EventoSistema::Irq { sequencia }) => Some(sequencia),
            _ => None,
        }
    }
    if sistema::cpl() != 3 || sistema::enviar(sistema::resolver(SUPERVISOR), 0x790, 1, 0, 0, 0) != 0
    {
        falha();
        return;
    }
    match sistema::receber_com_prazo(contrato_sistema::prazo::FINITO, 500) {
        Ok(m) if m.origem == sistema::resolver(SUPERVISOR) && m.codigo == 0x791 && m.a == 1 => {}
        _ => {
            falha();
            return;
        }
    }
    imprimirln!("[teste-irq] SEM_AUTORIDADE CPL3");
    let rede = sistema::resolver(REDE);
    if sistema::enviar(rede, 0x792, 0, 0, 0, 0) != 0 {
        falha();
        return;
    }
    match sistema::receber_com_prazo(contrato_sistema::prazo::FINITO, 500) {
        Ok(m) if m.origem == rede && m.codigo == 0x793 && m.a == 1 => {}
        _ => {
            falha();
            return;
        }
    }
    imprimirln!("[teste-irq] REDE_PRONTA_PARA_BLOQUEAR CPL3");
    // Apenas recepcao e OUT2; a UART pertence ao terminal neste ensaio.
    unsafe {
        io::saida8(0x3fc, 0x0b);
        io::saida8(0x3f9, 1);
    }
    if sistema::armar_irq4() != 0 || sistema::armar_irq4() != erro::ESTADO {
        falha();
        return;
    }
    // O host ainda nao enviou estimulo; nao dormir com trabalho local.
    if unsafe { io::entrada8(0x3fd) } & 1 != 0 {
        falha();
        return;
    }
    #[cfg(feature = "teste-rz024")]
    if armar_janela_idle_rz024(1) != 0 {
        falha();
        return;
    }
    imprimirln!("[teste-irq] ARMADA CPL3");
    let Some(seq) = irq() else {
        falha();
        return;
    };
    imprimirln!("[teste-irq] IRQ_ENTREGUE seq={}", seq);
    if io::ler_serial() != Some(b'A')
        || sistema::confirmar_irq4(seq.wrapping_add(1)) != erro::ESTADO
    {
        falha();
        return;
    }
    if sistema::enviar(rede, 0x795, seq, 0, 0, 0) != 0 {
        falha();
        return;
    }
    let impostora = match sistema::esperar_irq_ipc(contrato_sistema::prazo::FINITO, 500) {
        Ok(sistema::EventoSistema::Ipc(m))
            if sistema::cpl() == 3
                && m.origem == rede
                && m.codigo == contrato_sistema::classe_evento::IRQ
                && m.a == seq
                && m.b == erro::PERMISSAO
                && m.c == 1
                && m.d == 0 =>
        {
            m
        }
        _ => {
            falha();
            return;
        }
    };
    imprimirln!(
        "[teste-irq] IPC_IMPOSTORA_REJEITADA seq={} codigo={} retorno={:#x} CPL3",
        impostora.a,
        impostora.codigo,
        impostora.b
    );
    imprimirln!("[teste-irq] JANELA CPL3");
    // O host insere B depois da drenagem, com a IRQ ainda mascarada.
    let inicio = sistema::tiques();
    while unsafe { io::entrada8(0x3fd) } & 1 == 0 {
        if sistema::tiques().wrapping_sub(inicio) > 500 {
            falha();
            return;
        }
        sistema::ceder();
    }
    let (antes, mascara_antes) = amostrar_irq4();
    let inicio = sistema::tiques();
    while sistema::tiques().wrapping_sub(inicio) < 50 {
        sistema::ceder();
    }
    let decorrido = sistema::tiques().wrapping_sub(inicio);
    let (depois, mascara_depois) = amostrar_irq4();
    if antes == 0
        || antes == u64::MAX
        || antes == erro::PERMISSAO
        || antes != depois
        || depois == erro::PERMISSAO
        || mascara_antes & 0x10 == 0
        || mascara_depois & 0x10 == 0
        || decorrido < 50
        || unsafe { io::entrada8(0x3fd) } & 1 == 0
    {
        falha();
        return;
    }
    imprimirln!("[teste-irq] MASCARA_SEM_TEMPESTADE CPL3");
    if sistema::confirmar_irq4(seq) != 0 || sistema::confirmar_irq4(seq) != erro::ESTADO {
        falha();
        return;
    }
    // A verificacao depois de rearmar preserva trabalho mesmo sem nova borda.
    if unsafe { io::entrada8(0x3fd) } & 1 == 0 || io::ler_serial() != Some(b'B') {
        falha();
        return;
    }
    imprimirln!("[teste-irq] IRQ_ORIGINAL_CONFIRMADA seq={} CPL3", seq);
    // A borda de B pode permanecer no PIC mesmo apos a leitura do byte.
    // Absorve avisos vazios antes de pedir C e limita tempo e quantidade.
    let inicio = sistema::tiques();
    let mut anterior = seq;
    let mut vazios = 0;
    while sistema::tiques().wrapping_sub(inicio) < 100 {
        match sistema::esperar_irq_ipc(contrato_sistema::prazo::IMEDIATO, 0) {
            Err(erro::AUSENTE) => sistema::ceder(),
            Ok(sistema::EventoSistema::Irq { sequencia }) => {
                vazios += 1;
                if vazios > 8
                    || sequencia == anterior
                    || io::ler_serial().is_some()
                    || sistema::confirmar_irq4(sequencia) != 0
                {
                    falha();
                    return;
                }
                anterior = sequencia;
            }
            _ => {
                falha();
                return;
            }
        }
    }
    imprimirln!("[teste-irq] REARMADA CPL3");
    loop {
        let Some(seq3) = irq() else {
            falha();
            return;
        };
        if seq3 == anterior {
            falha();
            return;
        }
        anterior = seq3;
        let byte = io::ler_serial();
        if sistema::confirmar_irq4(seq3) != 0 {
            falha();
            return;
        }
        match byte {
            Some(b'C') => break,
            None if vazios < 8 => {
                vazios += 1;
            }
            _ => {
                falha();
                return;
            }
        }
    }
    unsafe {
        io::saida8(0x3f9, 0);
    }
    imprimirln!("[teste-irq] ENTREGA_CONFIRMACAO_REARME CPL3");
}

#[cfg(feature = "teste-prazo-empate")]
fn testar_prazo_empate() {
    const TOKEN: u64 = 0x0505;
    let rede = sistema::resolver(REDE);
    if sistema::cpl() != 3 || sistema::enviar(rede, 0x7f0, TOKEN, 0, 0, 0) != 0 {
        imprimirln!("[teste-prazo-empate] FALHA_PREPARACAO");
        return;
    }
    let primeiro = sistema::receber_com_prazo(contrato_sistema::prazo::FINITO, 40);
    if primeiro.err() != Some(erro::TEMPO) {
        imprimirln!("[teste-prazo-empate] FALHA_PRIMEIRO_RETORNO");
        return;
    }
    // Reabre uma espera após TEMPO para receber a IPC real uma única vez. Ela
    // pode permanecer na FIFO ou acordar esta segunda espera diretamente.
    let segundo = sistema::receber_com_prazo(contrato_sistema::prazo::FINITO, 200);
    match segundo {
        Ok(m)
            if m.origem == rede
                && m.codigo == 0x7f1
                && m.a != 0
                && m.b == TOKEN
                && m.c == 0
                && m.d == 0
                && sistema::cpl() == 3 =>
        {
            imprimirln!(
                "[teste-prazo-empate] RESULTADO primeiro=TEMPO segundo=IPC origem={:#x} codigo={:#x} prazo={} token={:#x} CPL3",
                m.origem, m.codigo, m.a, m.b
            );
        }
        Ok(m) => imprimirln!(
            "[teste-prazo-empate] FALHA_IPC origem={:#x} codigo={:#x} a={} b={:#x}",
            m.origem,
            m.codigo,
            m.a,
            m.b
        ),
        Err(codigo) => imprimirln!(
            "[teste-prazo-empate] FALHA_SEGUNDO_RETORNO erro={:#x}",
            codigo
        ),
    }
}

#[cfg(feature = "teste-cancelamento-aplicacao")]
fn testar_cancelamento_aplicacao() {
    const ID_CANCELADO: u64 = 0x2401;
    const ID_ATIVO: u64 = 0x2402;
    const VALOR_CANCELADO: u64 = 0x1111;
    const VALOR_ATIVO: u64 = 0x2222;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum EstadoOperacao {
        Ativa,
        Cancelada,
        Concluida,
    }
    struct Operacao {
        id: u64,
        valor: u64,
        estado: EstadoOperacao,
    }
    enum AcaoResposta {
        Concluida { id: u64, valor: u64 },
        IgnoradaCancelada { id: u64 },
    }
    fn correlacionar(
        operacoes: &mut [Operacao; 2],
        rede: u64,
        mensagem: Mensagem,
    ) -> Result<AcaoResposta, u64> {
        if mensagem.origem != rede || mensagem.codigo != 0x7d3 || mensagem.c != 0 || mensagem.d != 0
        {
            return Err(erro::ESTADO);
        }
        let Some(operacao) = operacoes.iter_mut().find(|op| op.id == mensagem.a) else {
            return Err(erro::AUSENTE);
        };
        if operacao.valor != mensagem.b {
            return Err(erro::ESTADO);
        }
        match operacao.estado {
            EstadoOperacao::Ativa => {
                operacao.estado = EstadoOperacao::Concluida;
                Ok(AcaoResposta::Concluida {
                    id: operacao.id,
                    valor: mensagem.b,
                })
            }
            EstadoOperacao::Cancelada => Ok(AcaoResposta::IgnoradaCancelada { id: operacao.id }),
            EstadoOperacao::Concluida => Err(erro::ESTADO),
        }
    }

    fn falha(etapa: u64, a: u64, b: u64) {
        imprimirln!(
            "[teste-cancelamento] FALHA etapa={} a={:#x} b={:#x}",
            etapa,
            a,
            b
        );
    }

    let rede = sistema::resolver(REDE);
    let mut operacoes = [
        Operacao {
            id: ID_CANCELADO,
            valor: VALOR_CANCELADO,
            estado: EstadoOperacao::Ativa,
        },
        Operacao {
            id: ID_ATIVO,
            valor: VALOR_ATIVO,
            estado: EstadoOperacao::Ativa,
        },
    ];
    if sistema::cpl() != 3 {
        falha(1, ID_CANCELADO, 0);
        return;
    }
    for operacao in &operacoes {
        if sistema::enviar(rede, 0x7d0, operacao.id, operacao.valor, 0, 0) != 0 {
            falha(1, operacao.id, 0);
            return;
        }
        imprimirln!("[teste-cancelamento] ENVIO_ACEITO id={:#x}", operacao.id);
    }

    match sistema::receber_com_prazo(contrato_sistema::prazo::FINITO, 500) {
        Ok(m)
            if sistema::cpl() == 3
                && m.origem == rede
                && m.codigo == 0x7d1
                && m.a == ID_CANCELADO
                && m.b == ID_ATIVO
                && m.c == 2
                && m.d == 3 => {}
        Ok(m) => {
            falha(2, m.codigo, m.a);
            return;
        }
        Err(codigo) => {
            falha(2, codigo, 0);
            return;
        }
    }
    imprimirln!(
        "[teste-cancelamento] SERVIDOR_RECEBEU ids={:#x},{:#x} CPL3",
        ID_CANCELADO,
        ID_ATIVO
    );

    if operacoes
        .iter()
        .any(|operacao| operacao.estado != EstadoOperacao::Ativa)
    {
        falha(3, ID_CANCELADO, ID_ATIVO);
        return;
    }
    operacoes[0].estado = EstadoOperacao::Cancelada;
    imprimirln!(
        "[teste-cancelamento] CANCELADO_LOCAL id={:#x}",
        ID_CANCELADO
    );
    if sistema::enviar(rede, 0x7d2, operacoes[0].id, operacoes[1].id, 0, 0) != 0 {
        falha(3, operacoes[0].id, operacoes[1].id);
        return;
    }

    let resposta_ativa = match sistema::receber_com_prazo(contrato_sistema::prazo::FINITO, 500) {
        Ok(m) if sistema::cpl() == 3 => match correlacionar(&mut operacoes, rede, m) {
            Ok(AcaoResposta::Concluida { id, valor }) if id == ID_ATIVO && valor == VALOR_ATIVO => {
                (id, valor)
            }
            Ok(AcaoResposta::IgnoradaCancelada { id }) => {
                falha(4, id, 0);
                return;
            }
            Ok(AcaoResposta::Concluida { id, valor }) => {
                falha(4, id, valor);
                return;
            }
            Err(codigo) => {
                falha(4, codigo, m.a);
                return;
            }
        },
        Ok(m) => {
            falha(4, m.a, m.b);
            return;
        }
        Err(codigo) => {
            falha(4, codigo, 0);
            return;
        }
    };
    imprimirln!(
        "[teste-cancelamento] RESPOSTA_ATIVA id={:#x} valor={:#x}",
        resposta_ativa.0,
        resposta_ativa.1
    );

    let resposta_tardia = match sistema::receber_com_prazo(contrato_sistema::prazo::FINITO, 500) {
        Ok(m) if sistema::cpl() == 3 => match correlacionar(&mut operacoes, rede, m) {
            Ok(AcaoResposta::IgnoradaCancelada { id }) if id == ID_CANCELADO => id,
            Ok(AcaoResposta::Concluida { id, valor }) => {
                falha(5, id, valor);
                return;
            }
            Ok(AcaoResposta::IgnoradaCancelada { id }) => {
                falha(5, id, 0);
                return;
            }
            Err(codigo) => {
                falha(5, codigo, m.a);
                return;
            }
        },
        Ok(m) => {
            falha(5, m.a, m.b);
            return;
        }
        Err(codigo) => {
            falha(5, codigo, 0);
            return;
        }
    };
    if operacoes[0].estado != EstadoOperacao::Cancelada
        || operacoes[1].estado != EstadoOperacao::Concluida
    {
        falha(6, resposta_tardia, 0);
        return;
    }
    imprimirln!(
        "[teste-cancelamento] RESPOSTA_TARDIA_IGNORADA id={:#x} estado=cancelada",
        resposta_tardia
    );
    let caixa_vazia = matches!(
        sistema::receber_com_prazo(contrato_sistema::prazo::IMEDIATO, 0),
        Err(codigo) if codigo == erro::AUSENTE
    );
    if !caixa_vazia
        || operacoes[0].estado != EstadoOperacao::Cancelada
        || operacoes[1].estado != EstadoOperacao::Concluida
    {
        falha(7, ID_CANCELADO, ID_ATIVO);
        return;
    }
    imprimirln!("[teste-cancelamento] CANCELAMENTO_PRESERVADO");
}

#[cfg(feature = "teste-quota")]
fn testar_quota() {
    fn pausa() {
        let inicio = sistema::tiques();
        while sistema::tiques().wrapping_sub(inicio) < 300 {
            sistema::ceder();
        }
    }
    let rede = sistema::resolver(REDE);
    let supervisor = sistema::resolver(SUPERVISOR);
    let mut ok = sistema::cpl() == 3 && sistema::enviar(rede, 0x780, 0, 0, 0, 0) == 0;
    pausa();
    ok &= sistema::enviar(supervisor, 0x784, 0, 0, 0, 0) == 0;
    pausa();
    for n in 1..=15 {
        ok &= matches!(sistema::receber(), Some(m) if m.origem == rede && m.codigo == 0x781 && m.a == n && m.b == 0 && m.c == 0 && m.d == 0);
    }
    ok &= matches!(sistema::receber(), Some(m) if m.origem == supervisor && m.codigo == 0x785 && m.a == 1 && m.b == 0 && m.c == 0 && m.d == 0);
    ok &= sistema::receber().is_none();
    ok &= sistema::enviar(rede, 0x782, 0, 0, 0, 0) == 0;
    pausa();
    ok &=
        matches!(sistema::receber(), Some(m) if m.origem == rede && m.codigo == 0x783 && m.a == 1);
    ok &= sistema::enviar(supervisor, 0x786, 0, 0, 0, 0) == 0;
    pausa();
    ok &= matches!(sistema::receber(), Some(m) if m.origem == supervisor && m.codigo == 0x787 && m.a == 1);
    if ok {
        imprimirln!("[teste-quota] FIFO_RESERVA_LIBERACAO CPL3");
    } else {
        imprimirln!("[teste-quota] FALHA");
    }
}

#[cfg(feature = "teste-rx-quota")]
fn sonda_fifo_rx_quota(modo: u64) -> Result<bool, u64> {
    match sistema::chamar(0xff07, modo, 0, 0, 0, 0, 0) {
        0 => Ok(false),
        1 => Ok(true),
        codigo => Err(codigo),
    }
}

#[cfg(feature = "teste-rx-quota")]
fn iniciar_teste_rx_quota(teste: &mut TesteRxQuota) {
    if sistema::cpl() != 3 || teste.etapa != 0 {
        teste.falhar("estado_ou_cpl_invalido");
        return;
    }
    let rede = sistema::resolver(REDE);
    let supervisor = sistema::resolver(SUPERVISOR);
    if sistema::enviar(rede, 0x7b8, 18, 0, 0, 0) != 0
        || sistema::enviar(supervisor, 0x7bb, 1, 0, 0, 0) != 0
    {
        teste.falhar("preparacao_ipc_recusada");
        return;
    }
    let inicio = sistema::tiques();
    let mut rede_pronta = false;
    let mut supervisor_pronto = false;
    while !(rede_pronta && supervisor_pronto) {
        if sistema::tiques().wrapping_sub(inicio) >= 300 {
            teste.falhar("timeout_preparacao_servicos");
            return;
        }
        match sistema::receber_com_prazo(contrato_sistema::prazo::FINITO, 10) {
            Ok(mensagem)
                if mensagem.origem == rede
                    && mensagem.codigo == 0x7b9
                    && mensagem.a == 18
                    && mensagem.b == 3
                    && mensagem.c == 0
                    && mensagem.d == 0 =>
            {
                rede_pronta = true;
            }
            Ok(mensagem)
                if mensagem.origem == supervisor
                    && mensagem.codigo == 0x7bc
                    && mensagem.a == 1
                    && mensagem.b == 3
                    && mensagem.c == 0
                    && mensagem.d == 0 =>
            {
                supervisor_pronto = true;
            }
            Err(erro::TEMPO) => {}
            Ok(_) => {
                teste.falhar("ack_preparacao_invalido");
                return;
            }
            Err(codigo) => {
                teste.falhar("erro_recebimento_preparacao");
                let _ = codigo;
                return;
            }
        }
    }
    teste.etapa = 1;
    teste.inicio = sistema::tiques();
    imprimirln!("\n[teste-rx-quota] ARMADA ids=1..18 rede=CPL3 supervisor=CPL3 quota=15");
}

#[no_mangle]
#[link_section = ".text.entrada"]
#[allow(unreachable_code)]
pub unsafe extern "sysv64" fn _inicio_servico(informacoes: *const InfoServico) -> ! {
    let info = *informacoes;
    if info.assinatura != ASSINATURA_SERVICO
        || info.versao != VERSAO_ABI
        || info.id != TERMINAL
        || !referencia_valida(info.referencia)
        || id_referencia(info.referencia) != TERMINAL
    {
        sistema::sair();
    }
    io::iniciar_serial();
    #[cfg(feature = "teste-uart-rz025")]
    imprimirln!("[teste-uart-rz025] CONFIG FCR=0xc7 RX_LIMIAR=14");
    console::iniciar(info.tela);
    #[cfg(feature = "teste-prazo-idle")]
    testar_prazo_idle();
    imprimirln!(
        "RAIZ OS / {} / MICROKERNEL / x86_64",
        env!("CARGO_PKG_VERSION")
    );
    imprimirln!("Sistema operacional proprio, escrito em Rust.");
    imprimirln!(
        "[terminal] Servico isolado iniciado: CPL={}",
        sistema::cpl()
    );
    imprimirln!("[ok] Console, framebuffer e teclado fora do kernel.");
    imprimirln!("[ok] IPC int80; paginas por processo; privilegios por servico.");
    let mut teclado = teclado::Teclado::iniciar();
    imprimirln!("Digite ajuda. Use ping 10.0.2.2 para testar a rede.");
    imprimirln!("[teste] TERMINAL_PRONTO");
    imprimir!("raiz> ");
    let mut linha = [0u8; 256];
    let mut tamanho = 0;
    let mut ultimo_cr = false;
    let mut contexto = ContextoTerminal {
        referencia_guardada: None,
    };
    // O ensaio de IRQ possui seu proprio protocolo de armacao.
    let mut irq_ativa = false;
    #[cfg(not(feature = "teste-irq"))]
    {
        let resultado = sistema::armar_irq4();
        if resultado == 0 {
            io::interrupcao_serial(true);
            irq_ativa = true;
        } else {
            imprimirln!(
                "\n[terminal] IRQ4 indisponivel ({}); polling serial.",
                resultado
            );
        }
    }
    let mut trabalho_serial = true; // Checagem inicial depois de armar.
    #[cfg(feature = "teste-uart")]
    let mut janela_testada = false;
    #[cfg(feature = "teste-uart")]
    let mut sinalizar_rearme = false;
    let mut sequencia_irq = None;
    let mut ipc_pendente = None;
    #[cfg(feature = "comparacao-consultas-vazias")]
    let mut medicao_vazia = MedicaoConsultasVazias::nova();
    #[cfg(feature = "teste-rx-quota")]
    let mut teste_rx_quota = TesteRxQuota::NOVO;
    #[cfg(feature = "teste-ciclo-espera")]
    let mut snapshot_driver_ciclo_pendente = None;
    #[cfg(feature = "teste-ciclo-espera")]
    let mut snapshot_driver_gen4 = None;
    let mut priorizar_teclado = false;
    #[cfg(feature = "teste-irq-ipc")]
    let mut teste_irq_ipc_pronto = false;
    #[cfg(feature = "teste-irq-ipc")]
    let mut teste_irq_ipc_ack_pendente = false;
    #[cfg(feature = "teste-irq-ipc")]
    let mut teste_irq_ipc_lote_liberado = false;
    #[cfg(feature = "teste-irq-ipc")]
    let mut teste_irq_ipc_retencao = false;
    #[cfg(feature = "teste-seletor-irq-ipc")]
    let mut teste_seletor_irq_ipc_solicitado = false;
    #[cfg(feature = "teste-irq-fifo-cheia")]
    let mut teste_irq_fifo_cheia_solicitado = false;
    #[cfg(feature = "teste-irq-morte")]
    let mut teste_irq_morte_solicitado = false;
    #[cfg(feature = "teste-irq-morte")]
    let mut teste_irq_morte_armado = false;
    #[cfg(feature = "teste-irq-morte")]
    let mut teste_irq_morte_byte_visto = false;
    #[cfg(feature = "teste-irq-morte")]
    let mut teste_irq_morte_tique_inicio = 0u64;
    #[cfg(feature = "teste-uart-rz025")]
    let mut teste_uart_rz025_fase = 0u8; // 0 inativa, 1 byte, 2 rajada, 3/4 armando.
    #[cfg(feature = "teste-uart-rz025")]
    let mut teste_uart_rz025_quantidade = 0usize;
    #[cfg(feature = "teste-uart-rz025")]
    let mut teste_uart_rz025_iir_rda = 0u32;
    #[cfg(feature = "teste-uart-rz025")]
    let mut teste_uart_rz025_iir_timeout = 0u32;
    #[cfg(feature = "teste-uart-rz025")]
    let mut teste_uart_rz025_irq4 = 0u32;
    #[cfg(feature = "teste-uart-rz025")]
    let mut teste_uart_rz025_oe_visto = false;
    loop {
        #[cfg(feature = "teste-rx-quota")]
        if matches!(teste_rx_quota.etapa, 1 | 2) {
            if sistema::tiques().wrapping_sub(teste_rx_quota.inicio) >= 500 {
                teste_rx_quota.falhar("timeout_barreira_fifo");
            } else {
                let modo = if teste_rx_quota.etapa == 1 { 1 } else { 2 };
                match sonda_fifo_rx_quota(modo) {
                    Ok(true) if modo == 1 && !teste_rx_quota.fifo_rede_pronta => {
                        teste_rx_quota.fifo_rede_pronta = true;
                        imprimirln!(
                            "\n[teste-rx-quota] FIFO_REDE_PRONTA quantidade=15 origem={:#x}",
                            sistema::resolver(REDE)
                        );
                    }
                    Ok(true) if modo == 2 => {
                        imprimirln!(
                            "\n[teste-rx-quota] FIFO_CHEIA rede=15 supervisor=1 total=16 CPL3"
                        );
                        teste_rx_quota.etapa = 3;
                    }
                    Ok(_) => {}
                    Err(codigo) => {
                        teste_rx_quota.falhar(if codigo == erro::PERMISSAO {
                            "sonda_permissao"
                        } else {
                            "sonda_estado"
                        });
                    }
                }
            }
        }
        #[cfg(feature = "comparacao-consultas-vazias")]
        let mut houve_trabalho = false;
        for _ in 0..16 {
            #[cfg(feature = "teste-rx-quota")]
            if matches!(teste_rx_quota.etapa, 1 | 2) {
                break;
            }
            #[cfg(feature = "teste-seletor-irq-ipc")]
            if teste_seletor_irq_ipc_solicitado {
                break;
            }
            let recebida = match ipc_pendente.take() {
                Some(m) => Some(m),
                None => {
                    let recebida = sistema::receber();
                    #[cfg(feature = "comparacao-consultas-vazias")]
                    if medicao_vazia.ativa && recebida.is_none() {
                        medicao_vazia.consultas_ipc_vazias =
                            medicao_vazia.consultas_ipc_vazias.saturating_add(1);
                    }
                    recebida
                }
            };
            let Some(m) = recebida else {
                break;
            };
            #[cfg(feature = "comparacao-consultas-vazias")]
            {
                houve_trabalho = true;
            }
            #[cfg(feature = "teste-rx-quota")]
            if (3..=8).contains(&teste_rx_quota.etapa)
                && matches!(m.codigo, 0x7ba | 0x7be | 0x7c2 | 0x7c3 | 0x7c6)
            {
                teste_rx_quota.tratar_ipc(&m);
                continue;
            }
            if m.origem == sistema::resolver(SUPERVISOR) {
                receber_supervisor(&m);
                continue;
            }
            if m.origem != sistema::resolver(REDE) {
                continue;
            }
            #[cfg(feature = "teste-rx-orcamento")]
            if (0x7c1..=0x7cf).contains(&m.codigo) {
                if sistema::cpl() != 3 {
                    imprimirln!("\n[teste-rx-orcamento] FALHA CPL_INVALIDO");
                    continue;
                }
                match m.codigo {
                    0x7c1 if m.a == 17 && m.d == 3 => imprimirln!(
                        "\n[teste-rx-orcamento] ARMADA quantidade={} rx_base={} quadros_base={} CPL3",
                        m.a,
                        m.b,
                        m.c
                    ),
                    0x7c3
                        if m.a == 16
                            && m.b == 16
                            && m.c & !0x1ffff == 0
                            && m.c.count_ones() == 16
                            && m.d == 1 =>
                    {
                        imprimirln!(
                            "\n[teste-rx-orcamento] LOTE descritores=16 teste=16 mascara={:#x} esgotado=1",
                            m.c
                        );
                    }
                    0x7c4 if m.a == 1 && m.b == 0 && m.c == 0 && m.d == 0 => {
                        imprimirln!("\n[teste-rx-orcamento] CEDER_RETORNOU");
                    }
                    0x7c5
                        if m.a == 1
                            && m.b == 1
                            && m.c & !0x1ffff == 0
                            && m.c.count_ones() == 1
                            && m.d == 0x1ffff =>
                    {
                        imprimirln!(
                            "\n[teste-rx-orcamento] CONTINUOU descritores=1 teste=1 mascara_nova={:#x} mascara_total={:#x} esgotado=0",
                            m.c,
                            m.d
                        );
                    }
                    0x7c6 if m.a == 17 && m.b < 32 && m.c == 0 && m.d == 0 => imprimirln!(
                        "\n[teste-rx-orcamento] DD_PRONTOS descritores=17 indice_rx={}",
                        m.b
                    ),
                    0x7cf if m.a == 4 => imprimirln!(
                        "\n[teste-rx-orcamento] FALHA fase=4 descritores={} teste={} mascara={:#x}",
                        m.b,
                        m.c,
                        m.d
                    ),
                    0x7cf if m.a == 5 => imprimirln!(
                        "\n[teste-rx-orcamento] FALHA fase=5 descritores={} teste={} mascara_total={:#x}",
                        m.b,
                        m.c,
                        m.d
                    ),
                    0x7cf => imprimirln!(
                        "\n[teste-rx-orcamento] FALHA fase={} a={} b={} c={}",
                        m.a,
                        m.b,
                        m.c,
                        m.d
                    ),
                    _ => imprimirln!(
                        "\n[teste-rx-orcamento] FALHA_DADOS codigo={:#x}",
                        m.codigo
                    ),
                }
                continue;
            }
            #[cfg(feature = "teste-irq-ipc")]
            if m.codigo == 0x78c {
                imprimirln!("\n[teste-irq-ipc] FALHA_PREPARACAO");
                continue;
            }
            #[cfg(feature = "teste-irq-ipc")]
            if m.codigo == 0x78a {
                if m.a == 8 && m.b == 0 && m.c == 0 && m.d == 0 && sistema::cpl() == 3 {
                    teste_irq_ipc_ack_pendente = true;
                } else {
                    imprimirln!("\n[teste-irq-ipc] FALHA_PREPARACAO");
                }
                continue;
            }
            #[cfg(feature = "teste-irq-ipc")]
            if m.codigo == 0x789 {
                if (1..=8).contains(&m.a) && m.b == 0 && m.c == 0 && m.d == 0 && sistema::cpl() == 3
                {
                    if trabalho_serial {
                        imprimirln!("\n[teste-irq-ipc] IPC_DURANTE_TRABALHO_SERIAL seq={}", m.a);
                    } else {
                        imprimirln!("\n[teste-irq-ipc] IPC_SEM_TRABALHO_SERIAL seq={}", m.a);
                    }
                } else {
                    imprimirln!("\n[teste-irq-ipc] FALHA_IPC codigo={:#x}", m.codigo);
                }
                continue;
            }
            #[cfg(feature = "teste-rz022")]
            if (0x7b1..=0x7b2).contains(&m.codigo) {
                if m.origem != sistema::resolver(REDE) || sistema::cpl() != 3 {
                    imprimirln!("\n[teste-rz022-rede] FALHA_ORIGEM");
                    continue;
                }
                match m.codigo {
                    0x7b1 if m.c == 10 && m.d == 0 => imprimirln!(
                        "\n[teste-rz022-rede] ANTES_ESPERA tique={} recebidos={} prazo={}",
                        m.a,
                        m.b,
                        m.c
                    ),
                    0x7b2 if m.d == 0 => imprimirln!(
                        "\n[teste-rz022-rede] RX_PROGREDIU recebidos={} ecos={} tique={}",
                        m.a,
                        m.b,
                        m.c
                    ),
                    _ => imprimirln!("\n[teste-rz022-rede] FALHA_DADOS codigo={:#x}", m.codigo),
                }
                imprimir!("raiz> ");
                continue;
            }
            #[cfg(feature = "teste-rz022")]
            if (0x7b5..=0x7b8).contains(&m.codigo) {
                let referencia = sistema::resolver(REDE);
                if m.origem != referencia || sistema::cpl() != 3 {
                    imprimirln!("\n[teste-rz022-rede] FALHA_ORIGEM_FIFO");
                    continue;
                }
                match m.codigo {
                    0x7b5
                        if (1..=32).contains(&m.a) && m.b == referencia && m.c != 0 && m.d == 0 =>
                    {
                        imprimirln!(
                            "\n[teste-rz022-rede] FIFO_ACK seq={} referencia={:#x} tique={}",
                            m.a,
                            m.b,
                            m.c
                        );
                    }
                    0x7b6 if m.a == referencia && m.c == 500 && m.d == 0 => {
                        imprimirln!(
                            "\n[teste-rz022-rede] FIFO_JANELA referencia={:#x} inicio={} tiques={}",
                            m.a,
                            m.b,
                            m.c
                        );
                    }
                    0x7b7 if m.a == 16 && m.b == referencia && m.d == 0 => {
                        imprimirln!(
                            "\n[teste-rz022-rede] FIFO_STASH_CHEIO quantidade={} referencia={:#x} tique={}",
                            m.a,
                            m.b,
                            m.c
                        );
                    }
                    0x7b8 if m.a == 500 && m.b == referencia && m.d == 0 => {
                        imprimirln!(
                            "\n[teste-rz022-rede] FIFO_JANELA_FIM tiques={} referencia={:#x}",
                            m.c,
                            m.b
                        );
                    }
                    _ => imprimirln!(
                        "\n[teste-rz022-rede] FALHA_DADOS_FIFO codigo={:#x}",
                        m.codigo
                    ),
                }
                imprimir!("raiz> ");
                continue;
            }
            #[cfg(feature = "teste-ciclo-espera")]
            if matches!(m.codigo, 0x740 | 0x745) {
                let referencia = sistema::resolver(REDE);
                let correto = sistema::cpl() == 3
                    && m.origem == referencia
                    && m.a == referencia
                    && m.b != 0
                    && m.c != 0
                    && m.d < 32
                    && snapshot_driver_ciclo_pendente.is_none()
                    && (m.codigo != 0x745 || referencia == 0x400000002);
                if correto {
                    snapshot_driver_ciclo_pendente = Some(SnapshotDriverCicloPendente {
                        codigo_inicio: m.codigo,
                        referencia,
                        mmio: m.b,
                        arena_fisica: m.c,
                        indice_rx: m.d,
                    });
                } else {
                    imprimirln!("\n[teste-ciclo-espera] FALHA_DRIVER SNAPSHOT_INICIO");
                }
                continue;
            }
            #[cfg(feature = "teste-ciclo-espera")]
            if matches!(m.codigo, 0x746 | 0x747) {
                let codigo_inicio_esperado = if m.codigo == 0x746 { 0x740 } else { 0x745 };
                let Some(pendente) = snapshot_driver_ciclo_pendente.take() else {
                    imprimirln!("\n[teste-ciclo-espera] FALHA_DRIVER SNAPSHOT_FIM_SEM_INICIO");
                    continue;
                };
                let correto = m.origem == pendente.referencia
                    && pendente.codigo_inicio == codigo_inicio_esperado
                    && m.codigo
                        == if codigo_inicio_esperado == 0x740 {
                            0x746
                        } else {
                            0x747
                        }
                    && m.d == 0
                    && m.a < 32;
                if !correto {
                    imprimirln!("\n[teste-ciclo-espera] FALHA_DRIVER SNAPSHOT_FIM");
                    continue;
                }
                let snapshot = SnapshotDriverCiclo {
                    referencia: pendente.referencia,
                    mmio: pendente.mmio,
                    arena_fisica: pendente.arena_fisica,
                    indice_rx: pendente.indice_rx,
                    indice_tx: m.a,
                    recebidos: m.b,
                    transmitidos: m.c,
                };
                if codigo_inicio_esperado == 0x740 {
                    imprimirln!(
                        "\n[teste-ciclo-espera] DRIVER_ATIVO referencia={:#x} mmio={:#x} arena={:#x} rx_idx={} tx_idx={} rx={} tx={} CPL3",
                        snapshot.referencia,
                        snapshot.mmio,
                        snapshot.arena_fisica,
                        snapshot.indice_rx,
                        snapshot.indice_tx,
                        snapshot.recebidos,
                        snapshot.transmitidos
                    );
                    if snapshot.referencia == 0x400000002 {
                        snapshot_driver_gen4 = Some(snapshot);
                    }
                    if sistema::enviar(
                        snapshot.referencia,
                        0x743,
                        snapshot.referencia,
                        0x4359434c4f,
                        0,
                        0,
                    ) != 0
                    {
                        imprimirln!("\n[teste-ciclo-espera] FALHA_DRIVER ACK");
                    }
                } else if snapshot_driver_gen4 == Some(snapshot) {
                    imprimirln!(
                        "\n[teste-ciclo-espera] DRIVER_PRESERVADO referencia={:#x} mmio={:#x} arena={:#x} rx_idx={} tx_idx={} rx={} tx={} igual=1 CPL3",
                        snapshot.referencia,
                        snapshot.mmio,
                        snapshot.arena_fisica,
                        snapshot.indice_rx,
                        snapshot.indice_tx,
                        snapshot.recebidos,
                        snapshot.transmitidos
                    );
                } else {
                    imprimirln!("\n[teste-ciclo-espera] FALHA_DRIVER ESTADO_ALTERADO");
                }
                continue;
            }
            #[cfg(feature = "teste-ciclo-espera")]
            if m.codigo == 0x742 {
                if m.origem == sistema::resolver(REDE)
                    && m.b == 0
                    && m.c == 0
                    && m.d == 0
                    && m.a == geracao_referencia(m.origem)
                    && m.a != 4
                {
                    imprimirln!("\n[teste-ciclo] REDE_LIBERADA geracao={}", m.a);
                } else if m.origem == sistema::resolver(REDE)
                    && m.a == 4
                    && m.a == geracao_referencia(m.origem)
                    && m.b == 3
                    && m.d == 0
                {
                    imprimirln!(
                        "\n[teste-ciclo-espera] REDE_TEMPO referencia={:#x} geracao=4 tiques={}",
                        m.origem,
                        m.c
                    );
                } else {
                    imprimirln!("\n[teste-ciclo] FALHA_REDE geracao={} erro={}", m.a, m.b);
                }
                continue;
            }
            match m.codigo {
                evento::REDE_PRONTA => {
                    if m.a == 0 {
                        imprimirln!(
                            "\n[rede] SERVICO_PRONTO CPL={} IPv4=10.0.2.15 UDP=7777 geracao={}",
                            m.d,
                            geracao_referencia(m.origem)
                        );
                    } else {
                        imprimirln!("\n[rede] Falha ao inicializar driver e1000.");
                    }
                    imprimir!("raiz> ");
                }
                evento::PING_OK => {
                    let ip = (m.a as u32).to_be_bytes();
                    imprimirln!(
                        "\n[rede] PING OK {}.{}.{}.{} seq={}",
                        ip[0],
                        ip[1],
                        ip[2],
                        ip[3],
                        m.b
                    );
                    imprimir!("raiz> ");
                }
                evento::REDE_STATUS => {
                    imprimirln!("\nIPv4=10.0.2.15/24 gateway=10.0.2.2 / servico ring 3");
                    imprimirln!("RX={} TX={} ping_ok={} udp_eco={}", m.a, m.b, m.c, m.d);
                    imprimir!("raiz> ");
                }
                evento::REDE_ERRO => {
                    imprimirln!("\n[rede] Operacao recusada: codigo={}", m.a);
                    imprimir!("raiz> ");
                }
                evento::REDE_EM_LACO => {
                    imprimirln!("\n[rede] LOOP_SEM_CEDER");
                    imprimir!("raiz> ");
                }
                _ => {}
            }
        }
        // Alterne a prioridade sem retirar um byte que nao sera processado.
        priorizar_teclado = !priorizar_teclado;
        let tecla = if priorizar_teclado {
            teclado.ler()
        } else {
            None
        };
        #[cfg(feature = "teste-rx-quota")]
        let teste_rx_quota_barreira = matches!(teste_rx_quota.etapa, 1 | 2);
        #[cfg(not(feature = "teste-rx-quota"))]
        let teste_rx_quota_barreira = false;
        let serial =
            if tecla.is_none() && (!irq_ativa || trabalho_serial || teste_rx_quota_barreira) {
                io::ler_serial()
            } else {
                None
            };
        if let Some(b) = tecla.or(serial).or_else(|| teclado.ler()) {
            #[cfg(feature = "comparacao-consultas-vazias")]
            {
                houve_trabalho = true;
            }
            #[cfg(feature = "teste-uart-rz025")]
            if serial == Some(b) && matches!(teste_uart_rz025_fase, 1 | 2) {
                let limite = if teste_uart_rz025_fase == 1 { 1 } else { 8 };
                if teste_uart_rz025_quantidade >= limite {
                    imprimirln!(
                        "\n[teste-uart-rz025] FALHA_LIMITE fase={}",
                        teste_uart_rz025_fase
                    );
                    teste_uart_rz025_fase = 0;
                    continue;
                }
                teste_uart_rz025_quantidade += 1;
                let fase_nome = if teste_uart_rz025_fase == 1 {
                    "UNICO"
                } else {
                    "RAJADA"
                };
                imprimirln!(
                    "\n[teste-uart-rz025] RBR fase={} id={} valor={:#04x}",
                    fase_nome,
                    teste_uart_rz025_quantidade,
                    b
                );
                if teste_uart_rz025_quantidade == limite {
                    let erros = io::retirar_erros_serial();
                    teste_uart_rz025_oe_visto |= erros & 0x02 != 0;
                    imprimirln!("[teste-uart-rz025] FASE_FIM fase={} rbr={} lsr_oe={} irq4={} iir_rda={} iir_timeout={}", fase_nome, limite, teste_uart_rz025_oe_visto as u8, teste_uart_rz025_irq4, teste_uart_rz025_iir_rda, teste_uart_rz025_iir_timeout);
                    if teste_uart_rz025_fase == 2 {
                        imprimirln!("[teste-uart-rz025] FIM CPL3");
                    }
                    imprimir!("raiz> ");
                    teste_uart_rz025_fase = 0;
                }
                continue;
            }
            #[cfg(feature = "teste-irq-morte")]
            if teste_irq_morte_armado && serial == Some(b'A') {
                teste_irq_morte_byte_visto = true;
            }
            #[cfg(feature = "teste-irq-ipc")]
            if trabalho_serial && serial.is_some() {
                imprimirln!("\n[teste-irq-ipc] BYTE_IRQ");
            }
            if b == b'\n' && ultimo_cr {
                ultimo_cr = false;
                continue;
            }
            ultimo_cr = b == b'\r';
            match b {
                b'\r' | b'\n' => {
                    imprimirln!();
                    let comando = core::str::from_utf8(&linha[..tamanho]).unwrap_or("").trim();
                    executar(
                        comando,
                        &info,
                        &mut contexto,
                        #[cfg(feature = "teste-seletor-irq-ipc")]
                        &mut teste_seletor_irq_ipc_solicitado,
                        #[cfg(feature = "teste-irq-fifo-cheia")]
                        &mut teste_irq_fifo_cheia_solicitado,
                        #[cfg(feature = "teste-irq-morte")]
                        &mut teste_irq_morte_solicitado,
                        #[cfg(feature = "teste-uart-rz025")]
                        &mut teste_uart_rz025_fase,
                        #[cfg(feature = "comparacao-consultas-vazias")]
                        &mut medicao_vazia,
                        #[cfg(feature = "teste-rx-quota")]
                        &mut teste_rx_quota,
                    );
                    #[cfg(feature = "teste-irq-morte")]
                    if teste_irq_morte_solicitado {
                        teste_irq_morte_tique_inicio = sistema::tiques();
                    }
                    tamanho = 0;
                    imprimir!("raiz> ");
                }
                8 | 127 => {
                    if tamanho > 0 {
                        tamanho -= 1;
                        imprimir!("\x08 \x08");
                    }
                }
                3 => {
                    tamanho = 0;
                    imprimirln!("^C");
                    imprimir!("raiz> ");
                }
                12 => {
                    console::limpar();
                    tamanho = 0;
                    imprimir!("raiz> ");
                }
                32..=126 => {
                    if tamanho < linha.len() {
                        linha[tamanho] = b;
                        tamanho += 1;
                        imprimir!("{}", b as char);
                    }
                }
                _ => {}
            }
        }
        let erros = io::retirar_erros_serial();
        #[cfg(feature = "teste-uart-rz025")]
        if matches!(teste_uart_rz025_fase, 1 | 2) && erros & 0x02 != 0 {
            teste_uart_rz025_oe_visto = true;
        }
        if erros != 0 {
            imprimirln!("\n[terminal] Erros UART LSR={:#x}", erros);
        }
        if !irq_ativa {
            sistema::ceder();
            continue;
        }
        // Drene apenas quando houve IRQ ou trabalho da checagem de rearme.
        if trabalho_serial {
            if io::serial_pendente() {
                sistema::ceder();
                continue;
            }
            trabalho_serial = false;
        }
        if let Some(sequencia) = sequencia_irq.take() {
            #[cfg(feature = "teste-irq-morte")]
            if teste_irq_morte_armado && teste_irq_morte_byte_visto {
                imprimirln!(
                    "\n[teste-irq-morte] IRQ4_RECEBIDA seq={} byte=0x41 CPL3",
                    sequencia
                );
                // Falha deliberada após IRQ real; a sequência não é confirmada.
                unsafe { core::arch::asm!("ud2", options(nomem, nostack)) };
            }
            #[cfg(feature = "teste-irq-ipc")]
            if teste_irq_ipc_retencao {
                teste_irq_ipc_retencao = false;
                let inicio = sistema::tiques();
                while sistema::tiques().wrapping_sub(inicio) < 20 {
                    sistema::ceder();
                }
                imprimirln!("\n[teste-irq-ipc] RETENCAO_FIM");
            }
            #[cfg(feature = "teste-uart")]
            if !janela_testada {
                janela_testada = true;
                sinalizar_rearme = true;
                imprimirln!("\n[teste-uart] JANELA_REARME");
                let inicio = sistema::tiques();
                while !io::serial_pendente() && sistema::tiques().wrapping_sub(inicio) < 500 {
                    sistema::ceder();
                }
                let pendente = io::serial_pendente();
                imprimirln!("[teste-uart] ANTES_CONFIRMACAO pendente={}", pendente as u8);
            }
            let resultado = sistema::confirmar_irq4(sequencia);
            if resultado != 0 {
                io::interrupcao_serial(false);
                irq_ativa = false;
                imprimirln!(
                    "\n[terminal] Falha ao confirmar IRQ4 ({}); polling serial.",
                    resultado
                );
                continue;
            }
            // Verificacao unica apos rearme cobre chegada na janela sem borda.
            trabalho_serial = io::serial_pendente();
            #[cfg(feature = "teste-uart")]
            if sinalizar_rearme {
                sinalizar_rearme = false;
                imprimirln!(
                    "[teste-uart] REARME_VERIFICADO pendente={}",
                    trabalho_serial as u8
                );
            }
            if trabalho_serial {
                continue;
            }
        }
        #[cfg(feature = "teste-irq-morte")]
        if teste_irq_morte_solicitado
            && sequencia_irq.is_none()
            && !trabalho_serial
            && !io::serial_pendente()
            && sistema::tiques().wrapping_sub(teste_irq_morte_tique_inicio) >= 2
            && sistema::chamar(0xff02, 0, 0, 0, 0, 0, 0) == 0
        {
            teste_irq_morte_solicitado = false;
            teste_irq_morte_armado = true;
            imprimirln!("\n[teste-irq-morte] ARMADA CPL3");
        }
        #[cfg(feature = "teste-irq-ipc")]
        if teste_irq_ipc_ack_pendente
            && sequencia_irq.is_none()
            && !trabalho_serial
            && !io::serial_pendente()
        {
            teste_irq_ipc_ack_pendente = false;
            teste_irq_ipc_pronto = true;
            imprimirln!("\n[teste-irq-ipc] CARGA_PRONTA CPL3");
        }
        #[cfg(feature = "teste-seletor-irq-ipc")]
        if teste_seletor_irq_ipc_solicitado
            && sequencia_irq.is_none()
            && !trabalho_serial
            && !io::serial_pendente()
        {
            teste_seletor_irq_ipc_solicitado = false;
            testar_irq_ipc();
        }
        #[cfg(feature = "teste-irq-fifo-cheia")]
        if teste_irq_fifo_cheia_solicitado
            && sequencia_irq.is_none()
            && !trabalho_serial
            && !io::serial_pendente()
        {
            teste_irq_fifo_cheia_solicitado = false;
            testar_irq_fifo_cheia();
        }
        #[cfg(feature = "teste-rx-quota")]
        if matches!(teste_rx_quota.etapa, 1 | 2) {
            // ESPERAR_IRQ_IPC também retira uma mensagem selecionada. Durante
            // a barreira T03-12 isso quebraria a composição exata da FIFO que
            // a sonda precisa observar. A serial é sondada por polling acima;
            // ceder permite que rede/supervisor progridam sem consumir IPC.
            sistema::ceder();
            continue;
        }
        match sistema::esperar_irq_ipc(contrato_sistema::prazo::FINITO, 1) {
            Ok(sistema::EventoSistema::Ipc(m)) => {
                ipc_pendente = Some(m);
                #[cfg(feature = "comparacao-consultas-vazias")]
                {
                    houve_trabalho = true;
                }
            }
            Ok(sistema::EventoSistema::Irq { sequencia }) => {
                #[cfg(feature = "teste-uart-rz025")]
                if matches!(teste_uart_rz025_fase, 1 | 2) {
                    teste_uart_rz025_irq4 = teste_uart_rz025_irq4.saturating_add(1);
                    let iir_bruto = io::iir_serial_bruto();
                    let causa = iir_bruto & 0x0e;
                    match causa {
                        0x04 => {
                            teste_uart_rz025_iir_rda = teste_uart_rz025_iir_rda.saturating_add(1)
                        }
                        0x0c => {
                            teste_uart_rz025_iir_timeout =
                                teste_uart_rz025_iir_timeout.saturating_add(1)
                        }
                        _ => {}
                    }
                    imprimirln!(
                        "\n[teste-uart-rz025] IRQ4 fase={} seq={} rbr_antes={} iir_raw={:#04x} iir_pending_bit={} causa={:#04x}",
                        if teste_uart_rz025_fase == 1 {
                            "UNICO"
                        } else {
                            "RAJADA"
                        },
                        sequencia,
                        teste_uart_rz025_quantidade,
                        iir_bruto,
                        iir_bruto & 1,
                        causa
                    );
                }
                sequencia_irq = Some(sequencia);
                trabalho_serial = true;
                #[cfg(feature = "teste-irq-ipc")]
                if teste_irq_ipc_pronto {
                    imprimirln!("\n[teste-irq-ipc] IRQ4 seq={}", sequencia);
                    if !teste_irq_ipc_lote_liberado {
                        teste_irq_ipc_lote_liberado = true;
                        teste_irq_ipc_retencao = true;
                        if sistema::enviar(sistema::resolver(REDE), 0x78b, 0, 0, 0, 0) != 0 {
                            imprimirln!("\n[teste-irq-ipc] FALHA_LIBERACAO");
                        }
                    }
                }
            }
            Err(contrato_sistema::erro::TEMPO) => {}
            Err(erro) => {
                io::interrupcao_serial(false);
                irq_ativa = false;
                imprimirln!(
                    "\n[terminal] Falha na espera IRQ4 ({}); polling serial.",
                    erro
                );
            }
            Ok(_) => {
                io::interrupcao_serial(false);
                irq_ativa = false;
                imprimirln!("\n[terminal] Evento inesperado; polling serial.");
            }
        }
        #[cfg(feature = "teste-uart-rz025")]
        if matches!(teste_uart_rz025_fase, 3 | 4)
            && sequencia_irq.is_none()
            && !trabalho_serial
            && !io::serial_pendente()
        {
            let fase = teste_uart_rz025_fase;
            let _ = io::retirar_erros_serial();
            teste_uart_rz025_oe_visto = false;
            teste_uart_rz025_quantidade = 0;
            teste_uart_rz025_iir_rda = 0;
            teste_uart_rz025_iir_timeout = 0;
            teste_uart_rz025_irq4 = 0;
            teste_uart_rz025_fase = if fase == 3 { 1 } else { 2 };
            imprimirln!(
                "\n[teste-uart-rz025] FASE_PRONTA fase={}",
                if fase == 3 { "UNICO" } else { "RAJADA" }
            );
        }
        #[cfg(feature = "comparacao-consultas-vazias")]
        {
            if medicao_vazia.ativa && !houve_trabalho {
                medicao_vazia.voltas_sem_trabalho =
                    medicao_vazia.voltas_sem_trabalho.saturating_add(1);
            }
            medicao_vazia.concluir_se_vencida();
        }
    }
}

#[cfg(feature = "teste-prazo-idle")]
fn testar_prazo_idle() -> ! {
    #[cfg(feature = "teste-rz024")]
    if armar_janela_idle_rz024(2) != 0 {
        imprimirln!("[teste-prazo-idle] FALHA_ARMAR_JANELA");
        sistema::sair()
    }
    let Ok(m) = sistema::receber_bloqueante() else {
        sistema::sair()
    };
    if m.origem != sistema::resolver(SUPERVISOR)
        || m.codigo != 0x732
        || m.a != 0xabc
        || m.b != 0xdef
        || m.c != 0x123
        || m.d != 0
        || sistema::cpl() != 3
    {
        imprimirln!("[teste-prazo-idle] FALHA_RESPOSTA");
        sistema::sair();
    }
    imprimirln!("[teste-prazo-idle] RESPOSTA_CPL3");
    loop {
        sistema::ceder();
    }
}

fn receber_supervisor(m: &Mensagem) {
    #[cfg(feature = "teste-rz022")]
    if (0x7a0..=0x7a2).contains(&m.codigo) {
        let referencia_supervisor = sistema::resolver(SUPERVISOR);
        if m.origem != referencia_supervisor || sistema::cpl() != 3 {
            imprimirln!("\n[teste-rz022-supervisor] FALHA_ORIGEM");
            return;
        }
        match m.codigo {
            0x7a0 if m.a >> 32 != 0 && matches!(m.c, 100 | 200 | 400) && m.d == 0 => {
                imprimirln!(
                    "\n[teste-rz022-supervisor] DEADLINE referencia={:#x} inicio={} atraso={}",
                    m.a,
                    m.b,
                    m.c
                );
            }
            0x7a1 if m.a >> 32 != 0 && m.d == 0 => {
                imprimirln!(
                    "\n[teste-rz022-supervisor] TENTATIVA referencia={:#x} tique={} indice={}",
                    m.a,
                    m.b,
                    m.c
                );
            }
            0x7a2 if m.a >> 32 != 0 && m.b >> 32 != 0 && m.c != 0 && m.d == 0 => {
                imprimirln!(
                    "\n[teste-rz022-supervisor] REINICIADO antiga={:#x} nova={:#x} tique={}",
                    m.a,
                    m.b,
                    m.c
                );
            }
            _ => imprimirln!(
                "\n[teste-rz022-supervisor] FALHA_DADOS codigo={:#x}",
                m.codigo
            ),
        }
        return;
    }
    #[cfg(feature = "teste-vida")]
    if m.codigo == 0x771 {
        if m.a <= 5 && m.b == 0 && m.c == 3 && m.d == 0 {
            imprimirln!("\n[teste-vida-servico] ETAPA {} CPL3", m.a);
        } else {
            imprimirln!("\n[teste-vida-servico] FALHA etapa={} erro={}", m.a, m.b);
        }
        return;
    }
    #[cfg(feature = "teste-observacao")]
    if m.codigo == 0x755 {
        if m.a <= 1 && m.b == 0 && m.c == 3 && m.d == 0 {
            imprimirln!("\n[teste-observacao] REFERENCIA etapa={} CPL3", m.a);
        } else {
            imprimirln!("\n[teste-observacao] FALHA_REFERENCIA");
        }
        return;
    }
    #[cfg(feature = "teste-observacao")]
    if m.codigo == 0x753 {
        if m.a == 0x100000002 && m.b == 0 && m.c == 3 && m.d == 0 {
            imprimirln!("\n[teste-observacao] SUPERVISOR_ALVO_INDISPONIVEL CPL3");
        } else {
            imprimirln!("\n[teste-observacao] FALHA_RESPOSTA");
        }
        return;
    }
    #[cfg(feature = "teste-prazo")]
    if m.codigo == 0x722 {
        if m.b != 0 || m.c != 0 || m.d != 0 || sistema::cpl() != 3 || !(1..=4).contains(&m.a) {
            imprimirln!("\n[teste-prazo-servico] FALHA_RESPOSTA");
        } else {
            imprimirln!("\n[teste-prazo-servico] ETAPA {} CPL3", m.a);
        }
        return;
    }
    #[cfg(feature = "teste-receber-bloqueante")]
    if m.codigo == 0x712 {
        if m.a == 1 && m.b == 2 && m.c == 3 && m.d == 0xabc && sistema::cpl() == 3 {
            imprimirln!("\n[teste-receber] RESPOSTA_FIFO_CPL3");
        } else {
            imprimirln!("\n[teste-receber] FALHA_RESPOSTA");
        }
        return;
    }
    #[cfg(feature = "teste-ipc-bloqueante")]
    if m.codigo == 0x702 {
        if m.a == 1 && m.b == 2 && m.c == 0xabc && m.d == 0 && sistema::cpl() == 3 {
            imprimirln!("\n[teste-ipc] RESPOSTA_FIFO_CPL3");
        } else {
            imprimirln!("\n[teste-ipc] FALHA_RESPOSTA");
        }
        return;
    }
    match m.codigo {
        evento::SUPERVISOR_PRONTO => {
            imprimirln!(
                "\n[supervisor] SERVICO_PRONTO CPL={} modo=manual limite={}",
                m.c,
                m.b
            );
        }
        evento::SUPERVISAO_ESTADO => {
            let modo = if m.a == 0 { "manual" } else { "auto" };
            #[cfg(feature = "teste-rz022")]
            if m.a == 1 && sistema::cpl() == 3 {
                imprimirln!(
                    "\n[teste-rz022-supervisor] AUTO_CONFIRMADO tique={}",
                    sistema::tiques()
                );
            }
            imprimirln!(
                "\n[supervisor] MODO {} reinicios={} ultimo_erro={:#x} limite={}",
                modo,
                m.b,
                m.c,
                m.d
            );
        }
        evento::SERVICO_REINICIADO => {
            imprimirln!(
                "\n[supervisor] REDE_REINICIADA geracao_antiga={} geracao_nova={} automatico={} referencia_antiga={:#x} referencia_nova={:#x}",
                geracao_referencia(m.a),
                geracao_referencia(m.b),
                m.c,
                m.a,
                m.b
            );
            imprimirln!("Nova instancia criada; aguardando inicializacao do driver.");
        }
        evento::SERVICO_ENCERRADO => {
            imprimirln!(
                "\n[supervisor] REDE_PARADA geracao={}",
                geracao_referencia(m.a)
            );
        }
        evento::SUPERVISOR_ERRO => {
            if m.a == erro::RECURSOS {
                imprimirln!(
                    "\n[supervisor] RECURSOS_ESGOTADOS operacao={} geracao={}",
                    m.b,
                    geracao_referencia(m.c)
                );
                imprimirln!("Limite de arenas/recuperacoes do boot atingido; consulte processos.");
            } else {
                imprimirln!(
                    "\n[supervisor] OPERACAO_RECUSADA codigo={:#x} operacao={} geracao={}",
                    m.a,
                    m.b,
                    geracao_referencia(m.c)
                );
            }
        }
        _ => return,
    }
    imprimir!("raiz> ");
}

fn enviar_rede(codigo: u64, a: u64) {
    let destino = sistema::resolver(REDE);
    match sistema::enviar(destino, codigo, a, 0, 0, 0) {
        0 => {}
        erro::SERVICO => {
            imprimirln!("Servico de rede indisponivel; terminal continua ativo.");
            imprimirln!("[teste] IPC_SERVICO_INDISPONIVEL_OK");
        }
        erro::REFERENCIA_ANTIGA => {
            imprimirln!("A rede mudou de geracao durante o pedido. Repita o comando.")
        }
        erro::FILA_CHEIA => imprimirln!("Fila IPC cheia. Tente novamente."),
        e => imprimirln!("IPC recusado: {:#x}", e),
    }
}

fn enviar_supervisor(codigo: u64, argumento: u64) {
    let destino = sistema::resolver(SUPERVISOR);
    match sistema::enviar(destino, codigo, argumento, 0, 0, 0) {
        0 => {}
        erro::SERVICO => imprimirln!("Supervisor indisponivel; a operacao nao foi executada."),
        erro::FILA_CHEIA => imprimirln!("Fila do supervisor cheia. Tente novamente."),
        e => imprimirln!("Pedido ao supervisor recusado: {:#x}", e),
    }
}

fn executar(
    comando: &str,
    info: &InfoServico,
    contexto: &mut ContextoTerminal,
    #[cfg(feature = "teste-seletor-irq-ipc")] teste_seletor_irq_ipc_solicitado: &mut bool,
    #[cfg(feature = "teste-irq-fifo-cheia")] teste_irq_fifo_cheia_solicitado: &mut bool,
    #[cfg(feature = "teste-irq-morte")] teste_irq_morte_solicitado: &mut bool,
    #[cfg(feature = "teste-uart-rz025")] teste_uart_rz025_fase: &mut u8,
    #[cfg(feature = "comparacao-consultas-vazias")] medicao_vazia: &mut MedicaoConsultasVazias,
    #[cfg(feature = "teste-rx-quota")] teste_rx_quota: &mut TesteRxQuota,
) {
    let mut partes = comando.split_whitespace();
    match partes.next().unwrap_or("") {
        "" => {}
        #[cfg(feature = "comparacao-consultas-vazias")]
        "medir-vazio" => medicao_vazia.armar(),
        #[cfg(feature = "teste-uart-rz025")]
        "teste-uart-rz025" => {
            if sistema::cpl() != 3 || *teste_uart_rz025_fase != 0 {
                imprimirln!("[teste-uart-rz025] FALHA_ARMA_FASE");
            } else {
                *teste_uart_rz025_fase = 3;
            }
        }
        #[cfg(feature = "teste-uart-rz025")]
        "teste-uart-rz025-rajada" => {
            if sistema::cpl() != 3 || *teste_uart_rz025_fase != 0 {
                imprimirln!("[teste-uart-rz025] FALHA_ARMA_FASE");
            } else {
                *teste_uart_rz025_fase = 4;
            }
        }
        #[cfg(feature = "teste-irq")]
        "teste-irq" => testar_irq(),
        #[cfg(feature = "teste-prazo-empate")]
        "teste-prazo-empate" => testar_prazo_empate(),
        #[cfg(feature = "teste-cancelamento-aplicacao")]
        "teste-cancelamento-aplicacao" => testar_cancelamento_aplicacao(),
        #[cfg(feature = "teste-irq-morte")]
        "teste-irq-morte" => {
            if sistema::cpl() != 3
                || sistema::enviar(sistema::resolver(SUPERVISOR), 0x7f0, 0, 0, 0, 0) != 0
            {
                imprimirln!("[teste-irq-morte] FALHA_PREPARACAO");
            } else {
                *teste_irq_morte_solicitado = true;
            }
        }
        #[cfg(feature = "teste-quota")]
        "teste-quota" => testar_quota(),
        #[cfg(feature = "teste-rx-quota")]
        "teste-rx-quota" => iniciar_teste_rx_quota(teste_rx_quota),
        #[cfg(feature = "teste-rx-quota")]
        "teste-rx-quota-controle" => {
            if teste_rx_quota.etapa != 1
                || !teste_rx_quota.fifo_rede_pronta
                || sistema::cpl() != 3
                || sistema::enviar(sistema::resolver(SUPERVISOR), 0x7bd, 1, 0, 0, 0) != 0
            {
                teste_rx_quota.falhar("controle_nao_armado");
            } else {
                teste_rx_quota.etapa = 2;
                teste_rx_quota.inicio = sistema::tiques();
            }
        }
        #[cfg(feature = "teste-vida")]
        "teste-vida-permissao" => {
            if sistema::inscrever_vida(sistema::resolver(REDE)).err() == Some(erro::PERMISSAO)
                && sistema::cpl() == 3
            {
                imprimirln!("[teste-vida-servico] PERMISSAO_NEGADA CPL3");
            } else {
                imprimirln!("[teste-vida-servico] FALHA_PERMISSAO");
            }
        }
        #[cfg(feature = "teste-vida")]
        "teste-vida-iniciar" => {
            if sistema::enviar(sistema::resolver(SUPERVISOR), 0x770, 0, 0, 0, 0) != 0 {
                imprimirln!("[teste-vida-servico] FALHA_INICIO");
            }
        }
        #[cfg(feature = "teste-vida")]
        "teste-vida-encher" => {
            let destino = sistema::resolver(SUPERVISOR);
            let mut correto = true;
            for n in 1..=16 {
                if sistema::enviar(destino, 0x772, n, 0xabc, 0, 0) != 0 {
                    correto = false;
                }
            }
            if sistema::enviar(destino, 0x772, 17, 0xabc, 0, 0) != erro::FILA_CHEIA {
                correto = false;
            }
            if correto {
                imprimirln!("[teste-vida-servico] FILA_CHEIA CONFIRMADA");
            } else {
                imprimirln!("[teste-vida-servico] FALHA_FILA");
            }
        }
        #[cfg(feature = "teste-rz022")]
        "teste-rz022-rede" => {
            let destino = sistema::resolver(REDE);
            if sistema::enviar(destino, 0x7b0, 0, 0, 0, 0) != 0 {
                imprimirln!("[teste-rz022-rede] FALHA_ATIVACAO");
            }
        }
        #[cfg(feature = "teste-rz022")]
        "teste-rz022-fifo-iniciar" => {
            let destino = sistema::resolver(REDE);
            for sequencia in 1..=16u64 {
                if sistema::enviar(destino, 0x7b4, sequencia, 0x022, 0, 0) != 0 {
                    imprimirln!("[teste-rz022-rede] FALHA_ENVIO_FIFO seq={}", sequencia);
                    return;
                }
            }
            imprimirln!("[teste-rz022-rede] FIFO_PRIMEIRA_ENVIADA quantidade=16");
        }
        #[cfg(feature = "teste-rz022")]
        "teste-rz022-fifo-continuar" => {
            let destino = sistema::resolver(REDE);
            for sequencia in 17..=32u64 {
                if sistema::enviar(destino, 0x7b4, sequencia, 0x022, 0, 0) != 0 {
                    imprimirln!("[teste-rz022-rede] FALHA_ENVIO_FIFO seq={}", sequencia);
                    return;
                }
            }
            if sistema::enviar(destino, 0x7b4, 33, 0x022, 0, 0) != erro::FILA_CHEIA {
                imprimirln!("[teste-rz022-rede] FALHA_FILA_CHEIA");
            } else {
                imprimirln!("[teste-rz022-rede] FILA_CHEIA_CONFIRMADA seq=33");
            }
        }
        #[cfg(feature = "teste-irq-ipc")]
        "teste-irq-ipc" => {
            if sistema::cpl() != 3
                || sistema::enviar(sistema::resolver(REDE), 0x788, 0, 0, 0, 0) != 0
            {
                imprimirln!("[teste-irq-ipc] FALHA_SOLICITACAO");
            }
        }
        #[cfg(feature = "teste-seletor-irq-ipc")]
        "teste-seletor-irq-ipc" => {
            if sistema::cpl() != 3 {
                imprimirln!("[teste-seletor-irq-ipc] FALHA_SOLICITACAO");
                sistema::sair();
            }
            *teste_seletor_irq_ipc_solicitado = true;
        }
        #[cfg(feature = "teste-irq-fifo-cheia")]
        "teste-irq-fifo-cheia" => {
            if sistema::cpl() != 3 {
                imprimirln!("[teste-irq-fifo-cheia] FALHA_CPL");
                sistema::sair();
            }
            *teste_irq_fifo_cheia_solicitado = true;
        }
        #[cfg(feature = "teste-rx-orcamento")]
        "teste-rx-orcamento" => {
            if sistema::cpl() != 3
                || sistema::enviar(sistema::resolver(REDE), 0x7c0, 17, 0, 0, 0) != 0
            {
                imprimirln!("[teste-rx-orcamento] FALHA_SOLICITACAO");
            }
        }
        #[cfg(feature = "teste-rx-orcamento")]
        "teste-rx-orcamento-liberar" => {
            if sistema::cpl() != 3
                || sistema::enviar(sistema::resolver(REDE), 0x7c2, 17, 0, 0, 0) != 0
            {
                imprimirln!("[teste-rx-orcamento] FALHA_LIBERACAO");
            }
        }
        #[cfg(feature = "teste-observacao")]
        "teste-observar-negado" => {
            let resultado = sistema::receber_observando(
                sistema::resolver(REDE),
                contrato_sistema::prazo::IMEDIATO,
                0,
            );
            if resultado.err() == Some(erro::PERMISSAO) && sistema::cpl() == 3 {
                imprimirln!("[teste-observacao] PERMISSAO_NEGADA CPL3");
            } else {
                imprimirln!("[teste-observacao] FALHA_PERMISSAO");
            }
        }
        #[cfg(feature = "teste-observacao")]
        "teste-referencia-observada" => {
            let etapa = match partes.next() {
                Some("0") => 0,
                Some("1") => 1,
                _ => 2,
            };
            if etapa > 1
                || sistema::enviar(sistema::resolver(SUPERVISOR), 0x754, etapa, 0, 0, 0) != 0
            {
                imprimirln!("[teste-observacao] FALHA_ENVIO_REFERENCIA");
            }
        }
        #[cfg(feature = "teste-observacao")]
        "teste-observar" => {
            if sistema::enviar(sistema::resolver(SUPERVISOR), 0x751, 0, 0, 0, 0) != 0 {
                imprimirln!("[teste-observacao] FALHA_ENVIO_SUPERVISOR");
            }
        }
        #[cfg(feature = "teste-observacao")]
        "teste-falhar-observado" => {
            if sistema::enviar(sistema::resolver(REDE), 0x752, 0, 0, 0, 0) != 0 {
                imprimirln!("[teste-observacao] FALHA_ENVIO_REDE");
            }
        }
        #[cfg(feature = "teste-ciclo-espera")]
        "teste-ciclo-liberar" => {
            let rede = sistema::resolver(REDE);
            let resultado =
                sistema::enviar(rede, 0x741, geracao_referencia(rede), 0xabc, 0xdef, 0x123);
            if resultado != 0 {
                imprimirln!("[teste-ciclo] FALHA_ENVIO {}", resultado);
            }
        }
        #[cfg(feature = "teste-prazo")]
        "teste-prazo-enviar" => {
            let sequencia = match partes.next() {
                Some("1") => 1,
                Some("2") => 2,
                _ => 0,
            };
            if sequencia == 0 || partes.next().is_some() {
                imprimirln!("[teste-prazo-servico] FALHA_COMANDO");
            } else {
                let resultado = sistema::enviar(
                    sistema::resolver(SUPERVISOR),
                    0x721,
                    sequencia,
                    0xabc,
                    0xdef,
                    0x123,
                );
                if resultado != 0 {
                    imprimirln!("[teste-prazo-servico] FALHA_ENVIO {}", resultado);
                }
            }
        }
        #[cfg(feature = "teste-receber-bloqueante")]
        "teste-receber-preparar" | "teste-receber-enviar" => {
            let destino = sistema::resolver(SUPERVISOR);
            let sequencias = if comando == "teste-receber-preparar" {
                1..=1
            } else {
                2..=3
            };
            for sequencia in sequencias {
                let codigo = if sequencia == 1 { erro::CHAMADA } else { 0x711 };
                let resultado = sistema::enviar(destino, codigo, sequencia, 0xabc, 0xdef, 0x123);
                if resultado != 0 {
                    imprimirln!("[teste-receber] FALHA_ENVIO {}", resultado);
                }
            }
        }
        #[cfg(feature = "teste-ipc-bloqueante")]
        "teste-ipc-bloqueante" => {
            let destino = sistema::resolver(SUPERVISOR);
            let primeiro = sistema::enviar(destino, 0x701, 1, 0xabc, 0, 0);
            let segundo = sistema::enviar(destino, 0x701, 2, 0xabc, 0, 0);
            if primeiro != 0 || segundo != 0 {
                imprimirln!("[teste-ipc] FALHA_ENVIO {} {}", primeiro, segundo);
            }
        }
        "ajuda" => {
            imprimirln!("sobre          - arquitetura e CPL do terminal");
            imprimirln!("memoria        - reservas e isolamento de enderecos");
            imprimirln!("processos      - estados dos servicos e tiques do timer");
            imprimirln!("rede           - pede estatisticas ao servico via IPC");
            imprimirln!("ping [IPv4]    - solicita ICMP ao servico de rede");
            imprimirln!("eco TEXTO      - imprime no console");
            imprimirln!("limpar         - limpa console e framebuffer");
            imprimirln!("teste-cap      - testa negacao de acesso PCI ao terminal");
            #[cfg(feature = "teste-rz022")]
            imprimirln!("teste-rz022-rede - verifica progresso RX apos anuncio do proximo prazo");
            #[cfg(feature = "teste-prazo-empate")]
            imprimirln!("teste-prazo-empate - compara timeout e IPC no mesmo tique logico");
            #[cfg(feature = "teste-rz022")]
            imprimirln!(
                "teste-rz022-fifo-iniciar / teste-rz022-fifo-continuar - testa IPC preservado no reset"
            );
            imprimirln!("teste-supervisor - terminal tenta reiniciar/parar sem direito");
            imprimirln!("falhar-rede    - provoca UD2 na rede; testa isolamento");
            imprimirln!("violar-memoria - rede tenta ler o nucleo; testa a MMU");
            imprimirln!("ocupar-rede    - loop infinito na rede; testa preempcao");
            imprimirln!("reiniciar-rede - supervisor cria nova instancia de rede");
            imprimirln!("parar-rede     - supervisor para a rede e muda para manual");
            imprimirln!("supervisao [auto|manual] - consulta ou altera a politica");
            imprimirln!("guardar-referencia - conserva a identidade atual da rede");
            imprimirln!("teste-referencia - testa envio e estado da identidade salva");
            imprimirln!("Limite: 4 instancias de rede por boot; arenas antigas em quarentena.");
        }
        "sobre" => {
            imprimirln!(
                "Raiz OS {} - microkernel proprio em Rust.",
                env!("CARGO_PKG_VERSION")
            );
            imprimirln!(
                "Terminal PID={} CPL={} / rede PID=2 / supervisor PID=3 / nucleo CPL=0",
                info.id,
                sistema::cpl()
            );
            imprimirln!("Nucleo: memoria, processos, IPC, interrupcoes e permissoes fixas.");
            imprimirln!("Servicos: terminal+console; rede+e1000; supervisor de recuperacao.");
            imprimirln!("Referencias incluem geracao; direitos sao verificados separadamente.");
            imprimirln!("Ainda sem arquivos persistentes, TCP ou IOMMU; driver DMA confiavel.");
        }
        "memoria" => {
            imprimirln!(
                "Memoria convencional recebida: {} MiB.",
                info.memoria_livre_boot / 1024 / 1024
            );
            imprimirln!(
                "Imagem deste processo: VA 0x40000000, segmentos {} KiB.",
                info.tamanho_imagem / 1024
            );
            imprimirln!("Reserva fisica: 2 MiB; paginas atribuidas conforme segmentos/pilha.");
            imprimirln!("Pilha propria; texto RX; dados RW+NX; nucleo supervisor.");
            imprimirln!("Tres servicos usam o mesmo VA com memoria fisica distinta.");
            imprimirln!("Reinicios da rede recebem arena nova; as antigas nao sao reutilizadas.");
        }
        "processos" => {
            imprimirln!(
                "terminal={} rede={} tiques={}",
                sistema::estado(TERMINAL),
                sistema::estado(REDE),
                sistema::tiques()
            );
            let referencia_rede = sistema::resolver(REDE);
            imprimirln!(
                "supervisor={} geracao_rede={} referencia_rede={:#x} referencia_terminal={:#x}",
                sistema::estado(SUPERVISOR),
                geracao_referencia(referencia_rede),
                referencia_rede,
                info.referencia
            );
            imprimirln!("Estados: 1=executavel 2=falhou 3=finalizado 0=inexistente.");
        }
        "limpar" => console::limpar(),
        "eco" => imprimirln!("{}", comando.strip_prefix("eco").unwrap_or("").trim_start()),
        "rede" => enviar_rede(evento::REDE_STATUS_SOLICITAR, 0),
        "ping" => {
            let endereco = partes.next().unwrap_or("10.0.2.2");
            if let Some(ip) = ler_ip(endereco) {
                imprimirln!("Solicitando ping a {} por IPC...", endereco);
                enviar_rede(evento::PING_SOLICITAR, u32::from_be_bytes(ip) as u64);
            } else {
                imprimirln!("IPv4 invalido. Exemplo: ping 10.0.2.2");
            }
        }
        "teste-cap" => {
            let resultado = sistema::chamar(chamada::PCI_LER, 0, 0, 0, 0, 0, 0);
            if resultado == erro::PERMISSAO {
                imprimirln!("[teste] CAPACIDADE_NEGADA_OK");
            } else {
                imprimirln!("[teste] ERRO_CAPACIDADE retorno={:#x}", resultado);
            }
        }
        "teste-supervisor" => {
            let referencia = sistema::resolver(REDE);
            let reinicio = sistema::reiniciar(referencia);
            let encerramento = sistema::encerrar(referencia);
            if reinicio == erro::PERMISSAO {
                imprimirln!("[teste] SUPERVISOR_REINICIAR_NEGADO_OK");
            } else {
                imprimirln!("[teste] ERRO_SUPERVISOR_REINICIAR retorno={:#x}", reinicio);
            }
            if encerramento == erro::PERMISSAO {
                imprimirln!("[teste] SUPERVISOR_ENCERRAR_NEGADO_OK");
            } else {
                imprimirln!(
                    "[teste] ERRO_SUPERVISOR_ENCERRAR retorno={:#x}",
                    encerramento
                );
            }
        }
        "reiniciar-rede" => {
            enviar_supervisor(evento::SERVICO_REINICIAR, sistema::resolver(REDE));
        }
        "parar-rede" => {
            enviar_supervisor(evento::SERVICO_ENCERRAR, sistema::resolver(REDE));
        }
        "supervisao" => match partes.next() {
            None => enviar_supervisor(evento::SUPERVISAO_CONSULTAR, 0),
            Some("auto") => {
                #[cfg(feature = "teste-rz022")]
                imprimirln!(
                    "[teste-rz022-supervisor] AUTO_SOLICITADO tique={}",
                    sistema::tiques()
                );
                enviar_supervisor(evento::SUPERVISAO_CONFIGURAR, 1)
            }
            Some("manual") => enviar_supervisor(evento::SUPERVISAO_CONFIGURAR, 0),
            _ => imprimirln!("Use: supervisao [auto|manual]"),
        },
        "guardar-referencia" => {
            let referencia = sistema::resolver(REDE);
            if referencia_valida(referencia) {
                contexto.referencia_guardada = Some(referencia);
                imprimirln!(
                    "[teste] REFERENCIA_GUARDADA geracao={} referencia={:#x}",
                    geracao_referencia(referencia),
                    referencia
                );
            } else {
                imprimirln!("Nao foi possivel resolver a referencia da rede.");
            }
        }
        "teste-referencia" => {
            if let Some(guardada) = contexto.referencia_guardada {
                // Não resolver novamente: este teste precisa apresentar ao kernel
                // exatamente a identidade conservada antes do reinício.
                let envio = sistema::enviar(guardada, evento::REDE_STATUS_SOLICITAR, 0, 0, 0, 0);
                let estado = sistema::estado_referencia(guardada);
                if envio == erro::REFERENCIA_ANTIGA {
                    imprimirln!("[teste] REFERENCIA_ANTIGA_ENVIO_OK");
                } else {
                    imprimirln!("[teste] ERRO_REFERENCIA_ANTIGA_ENVIO retorno={:#x}", envio);
                }
                if estado == erro::REFERENCIA_ANTIGA {
                    imprimirln!("[teste] REFERENCIA_ANTIGA_ESTADO_OK");
                } else {
                    imprimirln!(
                        "[teste] ERRO_REFERENCIA_ANTIGA_ESTADO retorno={:#x}",
                        estado
                    );
                }
            } else {
                imprimirln!("Use guardar-referencia, reinicie a rede e depois teste-referencia.");
            }
        }
        "falhar-rede" => {
            #[cfg(feature = "teste-rz022")]
            imprimirln!(
                "[teste-rz022-supervisor] FALHA_SOLICITADA referencia={:#x} tique={}",
                sistema::resolver(REDE),
                sistema::tiques()
            );
            imprimirln!("Solicitando falha controlada ao servico de rede...");
            enviar_rede(evento::REDE_FALHAR, 0);
        }
        "ocupar-rede" => {
            imprimirln!("Rede entrara em loop sem ceder; terminal depende de preempcao.");
            enviar_rede(evento::REDE_OCUPAR, 0);
        }
        "violar-memoria" => {
            imprimirln!("Rede tentara ler pagina supervisor; esperado: page fault.");
            enviar_rede(evento::REDE_ACESSAR_NUCLEO, 0);
        }
        cmd => imprimirln!("Comando desconhecido: {}. Digite ajuda.", cmd),
    }
}

fn ler_ip(s: &str) -> Option<[u8; 4]> {
    let mut ip = [0u8; 4];
    let mut partes = s.split('.');
    for octeto in &mut ip {
        *octeto = partes.next()?.parse().ok()?;
    }
    if partes.next().is_some() {
        None
    } else {
        Some(ip)
    }
}

#[panic_handler]
fn panico(info: &core::panic::PanicInfo) -> ! {
    use core::fmt::Write;
    let _ = writeln!(io::Emergencia, "\n[terminal] Panico: {}", info);
    sistema::sair()
}
