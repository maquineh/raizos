#![no_std]
#![no_main]

mod driver;

#[cfg(any(
    feature = "teste-rz022-rede",
    feature = "teste-rx-orcamento",
    feature = "teste-prazo-empate",
    feature = "teste-ciclo-espera"
))]
use apoio_servicos::tiques;
use apoio_servicos::{ceder, cpl, enviar, receber, receber_com_prazo, resolver, sair};
use contrato_sistema::{
    erro, evento, id_referencia, prazo, referencia_valida, InfoServico, ASSINATURA_SERVICO, REDE,
    TERMINAL, VERSAO_ABI,
};
use driver::Rede;

/// Eventos são valores fixos copiados pelo kernel, nunca ponteiros entre processos.
fn notificar(codigo: u64, a: u64, b: u64, c: u64, d: u64) {
    let destino = resolver(TERMINAL);
    if !referencia_valida(destino) {
        return;
    }
    for _ in 0..64 {
        let resultado = enviar(destino, codigo, a, b, c, d);
        if resultado != erro::FILA_CHEIA {
            break;
        }
        ceder();
    }
}

fn informar_erro(codigo: u64) {
    notificar(evento::REDE_ERRO, codigo, 0, 0, 0);
}

const PRAZO_POLL_RX: u64 = 10;
#[cfg(feature = "teste-rx-orcamento")]
const RX_ORCAMENTO_DESATIVADO: u8 = 0;
#[cfg(feature = "teste-rx-orcamento")]
const RX_ORCAMENTO_BLOQUEADO: u8 = 1;
#[cfg(feature = "teste-rx-orcamento")]
const RX_ORCAMENTO_PRIMEIRO_LOTE: u8 = 2;
#[cfg(feature = "teste-rx-orcamento")]
const RX_ORCAMENTO_CEDER: u8 = 3;
#[cfg(feature = "teste-rx-orcamento")]
const RX_ORCAMENTO_CONTINUACAO: u8 = 4;
#[cfg(feature = "teste-rx-orcamento")]
const RX_ORCAMENTO_CONCLUIDO: u8 = 5;
#[cfg(feature = "teste-rx-orcamento")]
const RX_ORCAMENTO_FALHOU: u8 = 6;
#[cfg(feature = "teste-rx-orcamento")]
const RX_ORCAMENTO_DESCRITORES_PRONTOS: u8 = 7;
#[cfg(feature = "teste-rx-orcamento")]
const RX_ORCAMENTO_PRAZO_DESCRITORES: u64 = 100;
#[cfg(feature = "teste-rx-orcamento")]
const RX_ORCAMENTO_MAX_TENTATIVAS_DESCRITORES: usize = 1024;
#[cfg(feature = "teste-rx-orcamento")]
const RX_ORCAMENTO_MASCARA_TOTAL: u32 = (1 << 17) - 1;
#[cfg(feature = "teste-cancelamento-aplicacao")]
const CANCELAMENTO_ID_CANCELADO: u64 = 0x2401;
#[cfg(feature = "teste-cancelamento-aplicacao")]
const CANCELAMENTO_ID_ATIVO: u64 = 0x2402;
#[cfg(feature = "teste-cancelamento-aplicacao")]
const CANCELAMENTO_VALOR_CANCELADO: u64 = 0x1111;
#[cfg(feature = "teste-cancelamento-aplicacao")]
const CANCELAMENTO_VALOR_ATIVO: u64 = 0x2222;

#[no_mangle]
#[link_section = ".text.entrada"]
#[allow(unreachable_code)]
pub extern "sysv64" fn _inicio_servico(informacoes: *const InfoServico) -> ! {
    if informacoes.is_null() || informacoes as usize % core::mem::align_of::<InfoServico>() != 0 {
        sair();
    }
    // SAFETY: página de informações somente leitura construída pelo microkernel.
    let info = unsafe { core::ptr::read_volatile(informacoes) };
    if info.assinatura != ASSINATURA_SERVICO
        || info.versao != VERSAO_ABI
        || info.id != REDE
        || !referencia_valida(info.referencia)
        || id_referencia(info.referencia) != REDE
    {
        sair();
    }
    #[cfg(feature = "teste-prazo-idle")]
    {
        let _ = apoio_servicos::receber_bloqueante();
        sair();
    }
    let mut rede = match unsafe { Rede::inicializar(info.base_fisica) } {
        Ok(rede) => rede,
        Err(_) => {
            notificar(evento::REDE_PRONTA, 1, 0, 0, cpl() as u64);
            sair();
        }
    };
    let estado = rede.estatisticas();
    let mac = estado
        .mac
        .iter()
        .fold(0u64, |numero, &byte| (numero << 8) | byte as u64);
    notificar(
        evento::REDE_PRONTA,
        0,
        mac,
        u32::from_be_bytes(estado.endereco_ipv4) as u64,
        cpl() as u64,
    );
    #[cfg(feature = "teste-ciclo-espera")]
    {
        if !enviar_snapshot_ciclo(0x740, 0x746, &rede, info.referencia)
            || !aguardar_ack_snapshot_ciclo(info.referencia)
        {
            sair();
        }
        if testar_ciclo_espera(info.referencia) {
            if !enviar_snapshot_ciclo(0x745, 0x747, &rede, info.referencia) {
                sair();
            }
        }
    }
    let mut respostas_notificadas = 0;
    let mut mensagem_pendente = None;
    let mut prazo_disponivel = true;
    #[cfg(feature = "teste-rz022-rede")]
    let mut teste_rx_ativo = false;
    #[cfg(feature = "teste-rz022-rede")]
    let mut rx_anterior_teste = 0;
    #[cfg(feature = "teste-rz022-rede")]
    let mut anunciar_espera_teste = false;
    #[cfg(feature = "teste-rx-orcamento")]
    let mut rx_orcamento_etapa = RX_ORCAMENTO_DESATIVADO;
    #[cfg(feature = "teste-rx-orcamento")]
    let mut rx_orcamento_recebidos_base = 0;
    #[cfg(feature = "teste-rx-orcamento")]
    let mut rx_orcamento_quadros_base = 0;
    #[cfg(feature = "teste-rx-orcamento")]
    let mut rx_orcamento_mascara_base = 0;
    #[cfg(feature = "teste-rx-orcamento")]
    let mut rx_orcamento_mascara_primeiro_lote = 0;
    #[cfg(feature = "teste-cancelamento-aplicacao")]
    let mut cancelamento_quantidade = 0;
    #[cfg(feature = "teste-cancelamento-aplicacao")]
    let mut cancelamento_ids = [0; 2];
    #[cfg(feature = "teste-cancelamento-aplicacao")]
    let mut cancelamento_valores = [0; 2];
    #[cfg(feature = "teste-cancelamento-aplicacao")]
    let mut cancelamento_liberado = false;
    #[cfg(feature = "teste-quota")]
    let mut resultado_quota = false;
    #[cfg(feature = "teste-rx-quota")]
    let (
        mut rx_quota_ativo,
        mut rx_quota_proximo,
        mut rx_quota_fila_cheia,
        mut rx_quota_tentado,
        mut rx_quota_retry_liberado,
        mut rx_quota_continuar_liberado,
        mut rx_quota_retries,
    ) = (false, 1u64, false, false, false, false, 0u64);
    loop {
        // Orçamento de IPC e RX impede que uma única rodada monopolize o serviço.
        for _ in 0..16 {
            let Some(mensagem) = mensagem_pendente
                .take()
                .or_else(driver::retirar_ipc_inicial)
                .or_else(receber)
            else {
                break;
            };
            if mensagem.origem != resolver(TERMINAL) {
                continue;
            }
            match mensagem.codigo {
                #[cfg(feature = "teste-rx-quota")]
                0x7b8 => {
                    let correto = !rx_quota_ativo
                        && mensagem.a == 18
                        && mensagem.b == 0
                        && mensagem.c == 0
                        && mensagem.d == 0
                        && cpl() == 3
                        && rede.armar_rx_quota();
                    if correto {
                        rx_quota_ativo = true;
                        notificar(0x7b9, 18, cpl() as u64, 0, 0);
                    } else {
                        notificar(0x7c6, 1, mensagem.a, mensagem.b, 0);
                    }
                }
                #[cfg(feature = "teste-rx-quota")]
                0x7bf => {
                    if rx_quota_ativo
                        && rx_quota_fila_cheia
                        && !rx_quota_retry_liberado
                        && mensagem.a == 16
                        && mensagem.b == 0x525a32355154
                        && mensagem.c == 0
                        && mensagem.d == 0
                        && cpl() == 3
                    {
                        rx_quota_retry_liberado = true;
                    } else {
                        notificar(0x7c6, 2, mensagem.a, mensagem.b, 0);
                    }
                }
                #[cfg(feature = "teste-rx-quota")]
                0x7c4 => {
                    if rx_quota_ativo
                        && cpl() == 3
                        && mensagem.a == 0
                        && mensagem.b == 0
                        && mensagem.c == 0
                        && mensagem.d == 0
                    {
                        let estado = rede.estatisticas();
                        let _ = enviar(
                            resolver(TERMINAL),
                            0x7c3,
                            estado.quadros_rx_quota,
                            estado.mascara_rx_quota as u64,
                            rx_quota_retries,
                            estado.rx_quota_invalido as u64,
                        );
                    } else {
                        notificar(0x7c6, 3, mensagem.a, mensagem.b, 0);
                    }
                }
                #[cfg(feature = "teste-rx-quota")]
                0x7c8 => {
                    if rx_quota_ativo
                        && rx_quota_proximo == 17
                        && !rx_quota_continuar_liberado
                        && mensagem.a == 16
                        && mensagem.b == 0x525a32355154
                        && mensagem.c == 0
                        && mensagem.d == 0
                        && cpl() == 3
                    {
                        rx_quota_continuar_liberado = true;
                    } else {
                        notificar(0x7c6, 4, mensagem.a, mensagem.b, 0);
                    }
                }
                #[cfg(feature = "teste-rx-orcamento")]
                0x7c0 => {
                    if rx_orcamento_etapa != RX_ORCAMENTO_DESATIVADO
                        || mensagem.a != 17
                        || mensagem.b != 0
                        || mensagem.c != 0
                        || mensagem.d != 0
                        || cpl() != 3
                    {
                        notificar(0x7cf, 1, 0, 0, 0);
                        continue;
                    }
                    let base = rede.estatisticas();
                    rx_orcamento_recebidos_base = base.recebidos;
                    rx_orcamento_quadros_base = base.quadros_rx_orcamento;
                    rx_orcamento_mascara_base = base.mascara_rx_orcamento;
                    rx_orcamento_etapa = RX_ORCAMENTO_BLOQUEADO;
                    notificar(
                        0x7c1,
                        17,
                        base.recebidos,
                        base.quadros_rx_orcamento,
                        cpl() as u64,
                    );
                    loop {
                        match apoio_servicos::receber_bloqueante() {
                            Ok(liberacao)
                                if liberacao.origem == resolver(TERMINAL)
                                    && liberacao.codigo == 0x7c2
                                    && liberacao.a == 17
                                    && liberacao.b == 0
                                    && liberacao.c == 0
                                    && liberacao.d == 0
                                    && cpl() == 3
                                    && rx_orcamento_etapa == RX_ORCAMENTO_BLOQUEADO =>
                            {
                                rx_orcamento_etapa = RX_ORCAMENTO_PRIMEIRO_LOTE;
                                break;
                            }
                            Ok(_) => {
                                notificar(0x7cf, 2, 0, 0, 0);
                            }
                            Err(codigo) => {
                                notificar(0x7cf, 3, codigo, 0, 0);
                                break;
                            }
                        }
                    }
                }
                #[cfg(feature = "teste-irq-ipc")]
                0x788 => {
                    let correto = cpl() == 3 && enviar(resolver(TERMINAL), 0x78a, 8, 0, 0, 0) == 0;
                    if !correto {
                        let _ = enviar(resolver(TERMINAL), 0x78c, 0, 0, 0, 0);
                    }
                }
                #[cfg(feature = "teste-irq-ipc")]
                0x78b => {
                    let mut correto = cpl() == 3;
                    for sequencia in 1..=8u64 {
                        correto &= enviar(resolver(TERMINAL), 0x789, sequencia, 0, 0, 0) == 0;
                        ceder();
                    }
                    if !correto {
                        let _ = enviar(resolver(TERMINAL), 0x78a, 0, 0, 0, 0);
                    }
                }
                #[cfg(feature = "teste-seletor-irq-ipc")]
                0x7e8 => {
                    let correto = cpl() == 3
                        && mensagem.origem == resolver(TERMINAL)
                        && mensagem.a == 0x21
                        && mensagem.b == 0
                        && mensagem.c == 0
                        && mensagem.d == 0
                        && enviar(resolver(TERMINAL), 0x7e9, 1, 0, 0, 0) == 0
                        && enviar(resolver(TERMINAL), 0x7e9, 2, 0, 0, 0) == 0;
                    if !correto {
                        let _ = enviar(resolver(TERMINAL), 0x7ea, 0, 0, 0, 0);
                    }
                }
                #[cfg(feature = "teste-irq-fifo-cheia")]
                0x7d8 => {
                    let mut correto = cpl() == 3
                        && mensagem.origem == resolver(TERMINAL)
                        && mensagem.a == 15
                        && mensagem.b == 0
                        && mensagem.c == 0
                        && mensagem.d == 0;
                    if correto {
                        for sequencia in 1..=15u64 {
                            if enviar(resolver(TERMINAL), 0x7da, sequencia, 0x11, 0x22, 0x33) != 0 {
                                correto = false;
                                break;
                            }
                        }
                    }
                    if !correto {
                        let _ = enviar(resolver(TERMINAL), 0x7dc, 1, 0, 0, 0);
                    }
                }
                #[cfg(feature = "teste-prazo-empate")]
                0x7f0 => {
                    if mensagem.a == 0x0505
                        && mensagem.b == 0
                        && mensagem.c == 0
                        && mensagem.d == 0
                        && cpl() == 3
                    {
                        testar_prazo_empate();
                    } else {
                        let _ = enviar(resolver(TERMINAL), 0x7ff, 1, 0, 0, 0);
                    }
                }
                #[cfg(feature = "teste-rz022-rede")]
                0x7b0 => {
                    teste_rx_ativo = true;
                    anunciar_espera_teste = true;
                    rx_anterior_teste = rede.estatisticas().recebidos;
                }
                #[cfg(feature = "teste-rz022-rede")]
                0x7b4 if (1..=32).contains(&mensagem.a) && mensagem.b == 0x022 => {
                    notificar(0x7b5, mensagem.a, resolver(REDE), tiques(), 0);
                }
                #[cfg(feature = "teste-irq")]
                0x792 => {
                    notificar(
                        0x793,
                        (cpl() == 3 && mensagem.origem == resolver(TERMINAL)) as u64,
                        0,
                        0,
                        0,
                    );
                }
                #[cfg(feature = "teste-irq")]
                0x795 if mensagem.a != 0 => {
                    // A sequencia e fornecida pelo terminal como dado de usuario.
                    // A tentativa de confirmar continua sujeita a autoridade do kernel.
                    let contexto_valido = cpl() == 3 && mensagem.origem == resolver(TERMINAL);
                    let confirmacao = apoio_servicos::confirmar_irq4(mensagem.a);
                    let envio = enviar(
                        resolver(TERMINAL),
                        contrato_sistema::classe_evento::IRQ,
                        mensagem.a,
                        confirmacao,
                        contexto_valido as u64,
                        0,
                    );
                    if !contexto_valido || confirmacao != erro::PERMISSAO || envio != 0 {
                        notificar(0x796, confirmacao, envio, contexto_valido as u64, 0);
                    }
                }
                #[cfg(feature = "teste-cancelamento-aplicacao")]
                0x7d0 => {
                    let esperado = match cancelamento_quantidade {
                        0 => Some((CANCELAMENTO_ID_CANCELADO, CANCELAMENTO_VALOR_CANCELADO)),
                        1 => Some((CANCELAMENTO_ID_ATIVO, CANCELAMENTO_VALOR_ATIVO)),
                        _ => None,
                    };
                    if cpl() != 3
                        || mensagem.b != esperado.map_or(0, |(_, valor)| valor)
                        || mensagem.c != 0
                        || mensagem.d != 0
                        || esperado.map(|(id, _)| id) != Some(mensagem.a)
                    {
                        notificar(0x7df, 1, mensagem.a, mensagem.b, 0);
                        continue;
                    }
                    cancelamento_ids[cancelamento_quantidade] = mensagem.a;
                    cancelamento_valores[cancelamento_quantidade] = mensagem.b;
                    cancelamento_quantidade += 1;
                    if cancelamento_quantidade == 2 {
                        notificar(
                            0x7d1,
                            cancelamento_ids[0],
                            cancelamento_ids[1],
                            cancelamento_quantidade as u64,
                            cpl() as u64,
                        );
                    }
                }
                #[cfg(feature = "teste-cancelamento-aplicacao")]
                0x7d2 => {
                    if cpl() != 3
                        || cancelamento_quantidade != 2
                        || cancelamento_liberado
                        || mensagem.a != cancelamento_ids[0]
                        || mensagem.b != cancelamento_ids[1]
                        || mensagem.c != 0
                        || mensagem.d != 0
                    {
                        notificar(0x7df, 2, mensagem.a, mensagem.b, 0);
                        continue;
                    }
                    cancelamento_liberado = true;
                    let resposta_ativa = enviar(
                        resolver(TERMINAL),
                        0x7d3,
                        cancelamento_ids[1],
                        cancelamento_valores[1],
                        0,
                        0,
                    );
                    let resposta_cancelada = enviar(
                        resolver(TERMINAL),
                        0x7d3,
                        cancelamento_ids[0],
                        cancelamento_valores[0],
                        0,
                        0,
                    );
                    if resposta_ativa != 0 || resposta_cancelada != 0 {
                        notificar(0x7df, 3, resposta_ativa, resposta_cancelada, 0);
                    }
                }
                #[cfg(feature = "teste-quota")]
                0x780 => {
                    resultado_quota = (1..=15)
                        .all(|n| enviar(resolver(TERMINAL), 0x781, n, 0, 0, 0) == 0)
                        && enviar(resolver(TERMINAL), 0x781, 16, 0, 0, 0) == erro::FILA_CHEIA;
                }
                #[cfg(feature = "teste-quota")]
                0x782 => {
                    let _ = enviar(resolver(TERMINAL), 0x783, resultado_quota as u64, 0, 0, 0);
                }
                #[cfg(feature = "teste-observacao")]
                0x752 => unsafe { core::arch::asm!("ud2", options(noreturn)) },
                evento::PING_SOLICITAR => {
                    if mensagem.a > u32::MAX as u64
                        || rede.ping((mensagem.a as u32).to_be_bytes()).is_err()
                    {
                        informar_erro(1);
                    }
                }
                evento::REDE_STATUS_SOLICITAR => {
                    let estado = rede.estatisticas();
                    notificar(
                        evento::REDE_STATUS,
                        estado.recebidos,
                        estado.transmitidos,
                        estado.respostas_ping,
                        estado.ecos_udp,
                    );
                }
                evento::REDE_FALHAR => {
                    // Ensaio solicitado pelo terminal: a falha deve encerrar só este processo.
                    unsafe {
                        core::arch::asm!("ud2", options(noreturn));
                    }
                }
                evento::REDE_ACESSAR_NUCLEO => {
                    // Ensaio de isolamento: esta VA aponta para página supervisor.
                    // A leitura deve causar #PF(14) e encerrar somente este serviço.
                    let _ = unsafe { core::ptr::read_volatile(0x0200_0000 as *const u8) };
                    informar_erro(999); // Acesso indevidamente autorizado, se retornar.
                }
                evento::REDE_OCUPAR => {
                    // Sem syscall CEDER: o PIT precisa preemptar este laço para o terminal viver.
                    notificar(evento::REDE_EM_LACO, 0, 0, 0, 0);
                    loop {
                        core::hint::spin_loop();
                    }
                }
                _ => {}
            }
        }
        #[cfg(feature = "teste-rx-orcamento")]
        if rx_orcamento_etapa == RX_ORCAMENTO_PRIMEIRO_LOTE {
            let inicio_tiques = tiques();
            let mut observou = false;
            for tentativa in 0..RX_ORCAMENTO_MAX_TENTATIVAS_DESCRITORES {
                let (prontos, indice_rx) = rede.rx_orcamento_descritores_prontos();
                if prontos == 17 {
                    rx_orcamento_etapa = RX_ORCAMENTO_DESCRITORES_PRONTOS;
                    notificar(0x7c6, prontos as u64, indice_rx as u64, 0, 0);
                    observou = true;
                    break;
                }
                let decorrido = tiques().wrapping_sub(inicio_tiques);
                if decorrido >= RX_ORCAMENTO_PRAZO_DESCRITORES
                    || tentativa + 1 == RX_ORCAMENTO_MAX_TENTATIVAS_DESCRITORES
                {
                    rx_orcamento_etapa = RX_ORCAMENTO_FALHOU;
                    notificar(0x7cf, 6, prontos as u64, indice_rx as u64, decorrido);
                    break;
                }
                // Espera cooperativa e limitada da imagem de ensaio; nenhum
                // caminho normal consulta ou altera o anel nesta observação.
                ceder();
            }
            if !observou && rx_orcamento_etapa == RX_ORCAMENTO_PRIMEIRO_LOTE {
                rx_orcamento_etapa = RX_ORCAMENTO_FALHOU;
                notificar(0x7cf, 6, 0, 0, 0);
            }
        }
        #[cfg(feature = "teste-rx-orcamento")]
        let estado_rx_antes = rede.estatisticas();
        let rx_orcamento_esgotado = rede.consultar();
        let estado = rede.estatisticas();
        #[cfg(feature = "teste-rx-quota")]
        if rx_quota_ativo && !estado.rx_quota_invalido {
            while rx_quota_proximo <= 18
                && estado.mascara_rx_quota & (1u32 << (rx_quota_proximo - 1)) != 0
            {
                if rx_quota_proximo >= 17 && !rx_quota_continuar_liberado {
                    break;
                }
                if rx_quota_proximo == 16 && rx_quota_tentado && !rx_quota_retry_liberado {
                    break;
                }
                let retry = rx_quota_proximo == 16 && rx_quota_retry_liberado;
                if retry {
                    rx_quota_retry_liberado = false;
                }
                let codigo = enviar(
                    resolver(TERMINAL),
                    0x7ba,
                    rx_quota_proximo,
                    0x525a32355154,
                    0,
                    0,
                );
                if codigo == 0 {
                    if retry {
                        rx_quota_retries += 1;
                    }
                    rx_quota_proximo += 1;
                    continue;
                }
                if codigo == erro::FILA_CHEIA && rx_quota_proximo == 16 {
                    rx_quota_fila_cheia = true;
                    rx_quota_tentado = true;
                    break;
                }
                rx_quota_fila_cheia = true;
                break;
            }
        }
        #[cfg(feature = "teste-rx-orcamento")]
        match rx_orcamento_etapa {
            RX_ORCAMENTO_DESCRITORES_PRONTOS => {
                let recebidos = estado.recebidos - estado_rx_antes.recebidos;
                let quadros = estado.quadros_rx_orcamento - estado_rx_antes.quadros_rx_orcamento;
                let mascara = estado.mascara_rx_orcamento & !rx_orcamento_mascara_base;
                if rx_orcamento_esgotado
                    && recebidos == 16
                    && quadros == 16
                    && mascara & !RX_ORCAMENTO_MASCARA_TOTAL == 0
                    && mascara.count_ones() == 16
                    && estado_rx_antes.recebidos >= rx_orcamento_recebidos_base
                    && estado_rx_antes.quadros_rx_orcamento >= rx_orcamento_quadros_base
                {
                    rx_orcamento_mascara_primeiro_lote = mascara;
                    rx_orcamento_etapa = RX_ORCAMENTO_CEDER;
                    notificar(0x7c3, recebidos, quadros, mascara as u64, 1);
                } else {
                    rx_orcamento_etapa = RX_ORCAMENTO_FALHOU;
                    notificar(0x7cf, 4, recebidos, quadros, mascara as u64);
                }
            }
            RX_ORCAMENTO_CONTINUACAO => {
                let recebidos = estado.recebidos - estado_rx_antes.recebidos;
                let quadros = estado.quadros_rx_orcamento - estado_rx_antes.quadros_rx_orcamento;
                let mascara_nova =
                    estado.mascara_rx_orcamento & !estado_rx_antes.mascara_rx_orcamento;
                let mascara_total = estado.mascara_rx_orcamento & !rx_orcamento_mascara_base;
                if !rx_orcamento_esgotado
                    && recebidos == 1
                    && quadros == 1
                    && mascara_nova & !RX_ORCAMENTO_MASCARA_TOTAL == 0
                    && mascara_nova.count_ones() == 1
                    && mascara_nova & rx_orcamento_mascara_primeiro_lote == 0
                    && estado.recebidos - rx_orcamento_recebidos_base == 17
                    && estado.quadros_rx_orcamento - rx_orcamento_quadros_base == 17
                    && mascara_total == RX_ORCAMENTO_MASCARA_TOTAL
                {
                    rx_orcamento_etapa = RX_ORCAMENTO_CONCLUIDO;
                    notificar(
                        0x7c5,
                        recebidos,
                        quadros,
                        mascara_nova as u64,
                        mascara_total as u64,
                    );
                } else {
                    rx_orcamento_etapa = RX_ORCAMENTO_FALHOU;
                    notificar(0x7cf, 5, recebidos, quadros, mascara_total as u64);
                }
            }
            _ => {}
        }
        #[cfg(feature = "teste-rz022-rede")]
        if teste_rx_ativo && estado.recebidos > rx_anterior_teste {
            notificar(0x7b2, estado.recebidos, estado.ecos_udp, tiques(), 0);
            rx_anterior_teste = estado.recebidos;
            anunciar_espera_teste = true;
        }
        if estado.respostas_ping != respostas_notificadas {
            respostas_notificadas = estado.respostas_ping;
            if let Some((destino, sequencia)) = rede.ultimo_ping() {
                notificar(
                    evento::PING_OK,
                    u32::from_be_bytes(destino) as u64,
                    sequencia as u64,
                    0,
                    0,
                );
            }
        }
        if rede.tomar_erro_ping() {
            informar_erro(1);
        }
        if rx_orcamento_esgotado {
            // A consulta consumiu todo o orçamento. Ceder mantém o serviço
            // elegível para a rodada seguinte, sem dormir com RX possivelmente pendente.
            ceder();
            #[cfg(feature = "teste-rx-orcamento")]
            if rx_orcamento_etapa == RX_ORCAMENTO_CEDER {
                // Marcador emitido somente depois que a syscall CEDER retornou.
                rx_orcamento_etapa = RX_ORCAMENTO_CONTINUACAO;
                notificar(0x7c4, 1, 0, 0, 0);
            }
            continue;
        }
        if prazo_disponivel {
            #[cfg(feature = "teste-rz022-rede")]
            if teste_rx_ativo && anunciar_espera_teste {
                let estado = rede.estatisticas();
                notificar(0x7b1, tiques(), estado.recebidos, PRAZO_POLL_RX, 0);
                anunciar_espera_teste = false;
            }
            match receber_com_prazo(prazo::FINITO, PRAZO_POLL_RX) {
                Ok(mensagem) => mensagem_pendente = Some(mensagem),
                Err(erro::TEMPO) => {}
                Err(erro::CHAMADA) => {
                    // Kernel ABI 2 antigo: polling residual cooperativo.
                    prazo_disponivel = false;
                }
                Err(codigo) => {
                    informar_erro(codigo);
                    prazo_disponivel = false;
                }
            }
        } else {
            // Fallback antigo sem suporte ao prazo: ceder e continuar consultando.
            ceder();
        }
    }
}

#[cfg(feature = "teste-prazo-empate")]
fn sondar_prazo_empate(referencia: u64) -> Result<Option<(u64, bool)>, u64> {
    let (resultado, prazo, referencia_saida, timeout): (u64, u64, u64, u64);
    // Sonda privada do kernel, disponível apenas na imagem opt-in. RDI entra
    // como referência do terminal e retorna prazo real; os registradores de
    // saída são validados para não confundir PERMISSAO/estado com prazo.
    unsafe {
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") 0xff06u64 => resultado,
            inlateout("rdi") referencia => prazo,
            lateout("rsi") referencia_saida,
            lateout("rdx") timeout,
        );
    }
    match resultado {
        0 if prazo == 0 && referencia_saida == referencia && timeout <= 1 => Ok(None),
        1 if referencia_saida == referencia && timeout <= 1 => Ok(Some((prazo, timeout != 0))),
        0 | 1 => Err(erro::ESTADO),
        codigo => Err(codigo),
    }
}

#[cfg(feature = "teste-prazo-empate")]
fn testar_prazo_empate() {
    const TOKEN: u64 = 0x0505;
    let terminal = resolver(TERMINAL);
    let inicio = tiques();
    let mut prazo_registrado = None;
    loop {
        if tiques().wrapping_sub(inicio) >= 500 {
            let _ = enviar(terminal, 0x7ff, 2, 0, 0, 0);
            return;
        }
        match sondar_prazo_empate(terminal) {
            Ok(Some((prazo, expirou))) => {
                prazo_registrado = Some(prazo);
                let agora = tiques();
                if expirou && agora.wrapping_sub(prazo) < (1u64 << 63) {
                    // Uma única admissão real após o timeout. Se o escalonador
                    // retomar a rede em tique posterior, o trace registra a
                    // diferença e o oráculo falha fechado.
                    let resultado = enviar(terminal, 0x7f1, prazo, TOKEN, 0, 0);
                    if resultado != 0 {
                        let _ = enviar(terminal, 0x7ff, resultado, prazo, 0, 0);
                    }
                    return;
                }
            }
            Ok(None) => {}
            Err(codigo) => {
                let _ = enviar(terminal, 0x7ff, codigo, 0, 0, 0);
                return;
            }
        }
        if let Some(prazo) = prazo_registrado {
            let agora = tiques();
            if agora.wrapping_sub(prazo) < (1u64 << 63) {
                // A expiração pode ser observada no próximo polling da sonda;
                // continuar sem ceder mantém o remetente pronto para enviar.
                core::hint::spin_loop();
            }
        }
        core::hint::spin_loop();
    }
}

#[cfg(feature = "teste-ciclo-espera")]
fn enviar_snapshot_ciclo(
    codigo_inicio: u64,
    codigo_fim: u64,
    rede: &Rede,
    referencia: u64,
) -> bool {
    let snapshot = rede.snapshot_ciclo_espera();
    let terminal = resolver(TERMINAL);
    let inicio = tiques();
    loop {
        let retorno = enviar(
            terminal,
            codigo_inicio,
            referencia,
            snapshot.mmio,
            snapshot.arena_fisica,
            snapshot.indice_rx,
        );
        if retorno == 0 {
            break;
        }
        if retorno != erro::FILA_CHEIA || tiques().wrapping_sub(inicio) >= 500 {
            return false;
        }
        ceder();
    }
    let inicio = tiques();
    loop {
        let retorno = enviar(
            terminal,
            codigo_fim,
            snapshot.indice_tx,
            snapshot.recebidos,
            snapshot.transmitidos,
            0,
        );
        if retorno == 0 {
            return true;
        }
        if retorno != erro::FILA_CHEIA || tiques().wrapping_sub(inicio) >= 500 {
            return false;
        }
        ceder();
    }
}

#[cfg(feature = "teste-ciclo-espera")]
fn aguardar_ack_snapshot_ciclo(referencia: u64) -> bool {
    match receber_com_prazo(prazo::FINITO, 500) {
        Ok(m) => {
            m.origem == resolver(TERMINAL)
                && m.codigo == 0x743
                && m.a == referencia
                && m.b == 0x4359434c4f
                && m.c == 0
                && m.d == 0
        }
        Err(_) => false,
    }
}

#[cfg(feature = "teste-ciclo-espera")]
fn testar_ciclo_espera(referencia: u64) -> bool {
    use contrato_sistema::{geracao_referencia, prazo};
    let geracao = geracao_referencia(referencia);
    let duracao = if geracao == 1 || geracao == 4 {
        1000
    } else {
        5000
    };
    let resultado = apoio_servicos::receber_com_prazo(prazo::FINITO, duracao);
    if geracao == 4 && resultado.as_ref().err() == Some(&erro::TEMPO) {
        if enviar(
            resolver(TERMINAL),
            0x742,
            geracao,
            3,
            apoio_servicos::tiques(),
            0,
        ) != 0
        {
            sair();
        }
        return true;
    }
    let Ok(m) = resultado else {
        let _ = enviar(resolver(TERMINAL), 0x742, geracao, 1, 0, 0);
        sair();
    };
    if geracao == 1
        || m.origem != resolver(TERMINAL)
        || m.codigo != 0x741
        || m.a != geracao
        || m.b != 0xabc
        || m.c != 0xdef
        || m.d != 0x123
    {
        let _ = enviar(resolver(TERMINAL), 0x742, geracao, 2, 0, 0);
        sair();
    }
    if enviar(resolver(TERMINAL), 0x742, geracao, 0, 0, 0) != 0 {
        sair();
    }
    false
}

#[panic_handler]
fn panico(_informacao: &core::panic::PanicInfo<'_>) -> ! {
    informar_erro(2);
    sair()
}
