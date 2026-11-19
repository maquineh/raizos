#![no_std]
#![no_main]
//! Política de recuperação em ring 3. O kernel apenas aplica direitos e mecanismos.
//! A supervisão automática observa falha/saída, não presume que um processo
//! executável esteja saudável. Não detecta laços sem progresso nesta versão.

use apoio_servicos as sistema;
use contrato_sistema::{
    chamada, erro, estado, evento, id_referencia, referencia_valida, InfoServico,
    ASSINATURA_SERVICO, MAX_INSTANCIAS_REDE, REDE, SUPERVISOR, TERMINAL, VERSAO_ABI,
};

const ATRASOS: [u64; 3] = [100, 200, 400];

struct Espera {
    referencia: u64,
    inicio: u64,
    atraso: u64,
}

struct Supervisao {
    automatica: bool,
    reinicios: u64,
    tentativas_automaticas: usize,
    ultimo_erro: u64,
    espera: Option<Espera>,
    #[cfg(feature = "teste-quota")]
    resultado_quota: bool,
    #[cfg(feature = "teste-rx-quota")]
    rx_quota_ativo: bool,
    #[cfg(feature = "teste-rx-quota")]
    rx_quota_resultados: Option<(u64, u64)>,
}

/// Uma fila cheia não pode prender indefinidamente a política de recuperação.
fn notificar(codigo: u64, a: u64, b: u64, c: u64, d: u64) {
    let destino = sistema::resolver(TERMINAL);
    if !referencia_valida(destino) {
        return;
    }
    for _ in 0..64 {
        if sistema::enviar(destino, codigo, a, b, c, d) != erro::FILA_CHEIA {
            return;
        }
        sistema::ceder();
    }
}

impl Supervisao {
    fn informar_estado(&self) {
        notificar(
            evento::SUPERVISAO_ESTADO,
            self.automatica as u64,
            self.reinicios,
            self.ultimo_erro,
            MAX_INSTANCIAS_REDE,
        );
    }

    fn informar_erro(&mut self, codigo: u64, operacao: u64, referencia: u64) {
        self.ultimo_erro = codigo;
        notificar(evento::SUPERVISOR_ERRO, codigo, operacao, referencia, 0);
    }

    fn reiniciar(&mut self, antiga: u64, automatico: bool) {
        self.espera = None;
        let nova = sistema::reiniciar(antiga);
        if referencia_valida(nova) && id_referencia(nova) == REDE {
            self.reinicios += 1;
            self.ultimo_erro = 0;
            notificar(
                evento::SERVICO_REINICIADO,
                antiga,
                nova,
                automatico as u64,
                0,
            );
            #[cfg(feature = "teste-rz022-supervisor")]
            notificar(0x7a2, antiga, nova, sistema::tiques(), 0);
        } else {
            self.informar_erro(nova, chamada::REINICIAR, antiga);
            // Sem recursos/permissão/imagem válida, repetir não ajudaria. A quota
            // de tentativas automáticas não é restaurada ao alternar o modo.
            if automatico && nova != erro::REFERENCIA_ANTIGA {
                self.automatica = false;
                self.informar_estado();
            }
        }
    }

    fn processar(&mut self, codigo: u64, argumento: u64) {
        match codigo {
            #[cfg(feature = "teste-irq-fifo-cheia")]
            0x7d9 if sistema::cpl() == 3 && argumento == 1 => {
                // O supervisor é o segundo remetente, preservando o espaço de
                // quota de rede e a ordem depois das 15 mensagens admitidas.
                let _ = sistema::enviar(sistema::resolver(TERMINAL), 0x7db, 1, 0x44, 0x55, 0x66);
            }
            #[cfg(feature = "teste-irq-morte")]
            0x7f0 if argumento == 0 && sistema::cpl() == 3 => {
                let terminal = sistema::resolver(TERMINAL);
                let inicio = sistema::tiques();
                while sistema::tiques().wrapping_sub(inicio) < 500 {
                    let estado_terminal = sistema::estado_referencia(terminal);
                    if estado_terminal == estado::FALHOU {
                        // Sinaliza após receber o retorno FALHOU em CPL3. O
                        // kernel aceita somente supervisor e estado/referência exatos.
                        if sistema::chamar(0xff03, terminal, 0, 0, 0, 0, 0) == 0 {
                            return;
                        }
                        return;
                    }
                    if estado_terminal != estado::EXECUTAVEL {
                        return;
                    }
                    let tique = sistema::tiques();
                    while sistema::tiques() == tique {
                        sistema::ceder();
                    }
                }
            }
            #[cfg(feature = "teste-irq")]
            0x790 => {
                let ok = sistema::cpl() == 3
                    && sistema::armar_irq4() == erro::PERMISSAO
                    && sistema::confirmar_irq4(argumento) == erro::PERMISSAO
                    && sistema::esperar_irq_ipc(contrato_sistema::prazo::IMEDIATO, 0).err()
                        == Some(erro::PERMISSAO);
                notificar(0x791, ok as u64, 0, 0, 0);
            }
            #[cfg(feature = "teste-quota")]
            0x784 => {
                let terminal = sistema::resolver(TERMINAL);
                self.resultado_quota = sistema::enviar(terminal, 0x785, 1, 0, 0, 0) == 0
                    && sistema::enviar(terminal, 0x785, 2, 0, 0, 0) == erro::FILA_CHEIA;
            }
            #[cfg(feature = "teste-quota")]
            0x786 => {
                let _ = sistema::enviar(
                    sistema::resolver(TERMINAL),
                    0x787,
                    self.resultado_quota as u64,
                    0,
                    0,
                    0,
                );
            }
            #[cfg(feature = "teste-rx-quota")]
            0x7bb if sistema::cpl() == 3 && self.rx_quota_resultados.is_none() => {
                if !self.rx_quota_ativo {
                    self.rx_quota_ativo = true;
                    let _ = sistema::enviar(
                        sistema::resolver(TERMINAL),
                        0x7bc,
                        1,
                        sistema::cpl() as u64,
                        0,
                        0,
                    );
                }
            }
            #[cfg(feature = "teste-rx-quota")]
            0x7bd if sistema::cpl() == 3 && self.rx_quota_ativo => {
                let terminal = sistema::resolver(TERMINAL);
                let primeiro = sistema::enviar(terminal, 0x7be, 1, 0x44, 0x55, 0x66);
                let segundo = sistema::enviar(terminal, 0x7be, 2, 0x44, 0x55, 0x66);
                self.rx_quota_resultados = Some((primeiro, segundo));
            }
            #[cfg(feature = "teste-rx-quota")]
            0x7c5 if sistema::cpl() == 3 && self.rx_quota_ativo => {
                if let Some((primeiro, segundo)) = self.rx_quota_resultados {
                    let _ = sistema::enviar(
                        sistema::resolver(TERMINAL),
                        0x7c2,
                        primeiro,
                        segundo,
                        sistema::cpl() as u64,
                        1,
                    );
                }
            }
            #[cfg(feature = "teste-observacao")]
            0x754 => {
                let esperado = if argumento == 0 {
                    erro::ALVO_INDISPONIVEL
                } else {
                    erro::REFERENCIA_ANTIGA
                };
                let resultado =
                    sistema::receber_observando(0x100000002, contrato_sistema::prazo::IMEDIATO, 0);
                let status = if resultado.err() == Some(esperado) {
                    0
                } else {
                    erro::ESTADO
                };
                notificar(0x755, argumento, status, sistema::cpl() as u64, 0);
            }
            #[cfg(feature = "teste-observacao")]
            0x751 => {
                let alvo = sistema::resolver(REDE);
                let resultado =
                    sistema::receber_observando(alvo, contrato_sistema::prazo::FINITO, 5000);
                let status = match resultado {
                    Err(erro::ALVO_INDISPONIVEL) => 0,
                    Err(e) => e,
                    Ok(_) => erro::ESTADO,
                };
                notificar(0x753, alvo, status, sistema::cpl() as u64, 0);
            }
            evento::SUPERVISAO_CONFIGURAR => {
                if argumento > 1 {
                    self.informar_erro(erro::ESTADO, codigo, 0);
                    return;
                }
                self.automatica = argumento == 1;
                self.espera = None;
                self.informar_estado();
            }
            evento::SUPERVISAO_CONSULTAR => self.informar_estado(),
            evento::SERVICO_REINICIAR => self.reiniciar(argumento, false),
            evento::SERVICO_ENCERRAR => {
                // Parar é uma intenção persistente: suspender auto antes da syscall.
                self.automatica = false;
                self.espera = None;
                let resultado = sistema::encerrar(argumento);
                if resultado == 0 {
                    self.ultimo_erro = 0;
                    notificar(evento::SERVICO_ENCERRADO, argumento, 0, 0, 0);
                } else {
                    self.informar_erro(resultado, chamada::ENCERRAR, argumento);
                }
                self.informar_estado();
            }
            _ => {}
        }
    }

    fn consultar(&mut self) {
        if !self.automatica {
            return;
        }
        let referencia = sistema::resolver(REDE);
        if !referencia_valida(referencia) {
            self.automatica = false;
            self.informar_erro(erro::SERVICO, chamada::RESOLVER, referencia);
            self.informar_estado();
            return;
        }
        let estado_atual = sistema::estado_referencia(referencia);
        match estado_atual {
            estado::EXECUTAVEL => {
                self.espera = None;
                return;
            }
            estado::FALHOU | estado::FINALIZADO => {}
            codigo => {
                // Um erro de consulta não prova que a rede está saudável. Sem
                // um estado reconciliado, parar a automação e esperar novo
                // comando evita reiniciar a geração errada ou dormir até sempre.
                let erro_consulta = match codigo {
                    erro::SERVICO | erro::REFERENCIA_ANTIGA | erro::ESTADO => codigo,
                    _ => erro::ESTADO,
                };
                self.automatica = false;
                self.espera = None;
                self.informar_erro(erro_consulta, chamada::ESTADO_SERVICO, referencia);
                self.informar_estado();
                return;
            }
        }
        if self.tentativas_automaticas >= ATRASOS.len() {
            self.automatica = false;
            self.espera = None;
            self.informar_erro(erro::RECURSOS, chamada::REINICIAR, referencia);
            self.informar_estado();
            return;
        }
        let agora = sistema::tiques();
        if let Some(espera) = &self.espera {
            if espera.referencia == referencia {
                if agora.wrapping_sub(espera.inicio) >= espera.atraso {
                    self.tentativas_automaticas += 1;
                    #[cfg(feature = "teste-rz022-supervisor")]
                    notificar(
                        0x7a1,
                        referencia,
                        agora,
                        self.tentativas_automaticas as u64,
                        0,
                    );
                    self.reiniciar(referencia, true);
                }
                return;
            }
        }
        let atraso = ATRASOS[self.tentativas_automaticas];
        self.espera = Some(Espera {
            referencia,
            inicio: agora,
            atraso,
        });
        #[cfg(feature = "teste-rz022-supervisor")]
        notificar(0x7a0, referencia, agora, atraso, 0);
    }

    /// Prazo restante até a próxima retentativa; a comparação permanece modular.
    fn prazo_espera(&self) -> Option<u64> {
        let espera = self.espera.as_ref()?;
        let decorrido = sistema::tiques().wrapping_sub(espera.inicio);
        Some(espera.atraso.saturating_sub(decorrido))
    }
}

#[no_mangle]
#[link_section = ".text.entrada"]
#[allow(unreachable_code)]
pub extern "sysv64" fn _inicio_servico(informacoes: *const InfoServico) -> ! {
    if informacoes.is_null() || informacoes as usize % core::mem::align_of::<InfoServico>() != 0 {
        sistema::sair();
    }
    let info = unsafe { core::ptr::read_volatile(informacoes) };
    if info.assinatura != ASSINATURA_SERVICO
        || info.versao != VERSAO_ABI
        || info.id != SUPERVISOR
        || !referencia_valida(info.referencia)
        || id_referencia(info.referencia) != SUPERVISOR
    {
        sistema::sair();
    }
    #[cfg(feature = "teste-prazo-idle")]
    testar_prazo_idle();
    let mut supervisao = Supervisao {
        automatica: false,
        reinicios: 0,
        tentativas_automaticas: 0,
        ultimo_erro: 0,
        espera: None,
        #[cfg(feature = "teste-quota")]
        resultado_quota: false,
        #[cfg(feature = "teste-rx-quota")]
        rx_quota_ativo: false,
        #[cfg(feature = "teste-rx-quota")]
        rx_quota_resultados: None,
    };
    notificar(
        evento::SUPERVISOR_PRONTO,
        0,
        MAX_INSTANCIAS_REDE,
        sistema::cpl() as u64,
        0,
    );
    #[cfg(feature = "teste-prazo-empate")]
    {
        // Remove o terceiro serviço da janela para que o teste controlado
        // tenha somente terminal (esperador) e rede (remetente) elegíveis.
        // O supervisor continua no CPL3 e consome qualquer IPC que o acorde.
        let _ = sistema::receber_bloqueante();
    }
    #[cfg(feature = "teste-ipc-bloqueante")]
    testar_ipc_bloqueante();
    #[cfg(feature = "teste-receber-bloqueante")]
    testar_receber_bloqueante();
    #[cfg(feature = "teste-prazo")]
    testar_prazo();
    #[cfg(feature = "teste-vida")]
    testar_vida();
    let mut vida_disponivel = true;
    let mut inscricao_vida = None;
    let mut espera_disponivel = true;
    let mut receber_bloqueante_disponivel = true;
    loop {
        if vida_disponivel && inscricao_vida.is_none() {
            let referencia = sistema::resolver(REDE);
            match sistema::inscrever_vida(referencia) {
                Ok(_estado_inicial) => {
                    inscricao_vida = Some(referencia);
                    // A inscrição é atômica com a leitura inicial. Reconcilia
                    // uma rede que já estava indisponível ao registrar o slot.
                    supervisao.consultar();
                }
                Err(erro::CHAMADA) => vida_disponivel = false,
                Err(e) => {
                    vida_disponivel = false;
                    supervisao.informar_erro(e, chamada::INSCREVER_VIDA, referencia);
                }
            }
        }
        for _ in 0..16 {
            if vida_disponivel && inscricao_vida.is_some() {
                match sistema::esperar_evento(contrato_sistema::prazo::IMEDIATO, 0) {
                    Ok(evento) => {
                        processar_evento_vida(&mut supervisao, &mut inscricao_vida, evento)
                    }
                    Err(erro::AUSENTE) => break,
                    Err(e) => {
                        desabilitar_vida(
                            &mut supervisao,
                            &mut vida_disponivel,
                            &mut inscricao_vida,
                            e,
                        );
                        break;
                    }
                }
                continue;
            }
            let Some(mensagem) = sistema::receber() else {
                break;
            };
            if mensagem.origem != sistema::resolver(TERMINAL) {
                continue;
            }
            supervisao.processar(mensagem.codigo, mensagem.a);
        }
        supervisao.consultar();
        if vida_disponivel && inscricao_vida.is_none() {
            // Reinscrever a geração publicada antes de voltar a dormir.
            continue;
        }
        if vida_disponivel && inscricao_vida.is_some() {
            let prazo = if supervisao.automatica {
                supervisao.prazo_espera()
            } else {
                None
            };
            let (modo, duracao) = match prazo {
                Some(restante) => (contrato_sistema::prazo::FINITO, restante.max(1)),
                None => (contrato_sistema::prazo::SEM_PRAZO, 0),
            };
            match sistema::esperar_evento(modo, duracao) {
                Ok(evento) => processar_evento_vida(&mut supervisao, &mut inscricao_vida, evento),
                // O timeout só acorda para reconciliar o estado e aplicar o
                // backoff; IPC e vida ficam pendentes no kernel.
                Err(erro::TEMPO) => {}
                Err(e) => desabilitar_vida(
                    &mut supervisao,
                    &mut vida_disponivel,
                    &mut inscricao_vida,
                    e,
                ),
            }
            continue;
        }
        // Fallback para kernels ABI 2 sem notificações persistentes. Neles
        // conserva-se o comportamento anterior baseado em consulta e ceder.
        if !supervisao.automatica && receber_bloqueante_disponivel {
            match sistema::receber_bloqueante() {
                Ok(mensagem) => {
                    if mensagem.origem == sistema::resolver(TERMINAL) {
                        supervisao.processar(mensagem.codigo, mensagem.a);
                    }
                    continue;
                }
                Err(resultado) => {
                    receber_bloqueante_disponivel = false;
                    if resultado != erro::CHAMADA {
                        supervisao.informar_erro(resultado, chamada::RECEBER_BLOQUEANTE, 0);
                    }
                }
            }
        }
        if supervisao.automatica || !espera_disponivel {
            sistema::ceder();
        } else {
            let resultado = sistema::esperar_ipc();
            if resultado != 0 {
                // Não repetir uma chamada indisponível ou que falhou. Em um
                // kernel anterior CHAMADA é o fallback esperado; outras falhas
                // são informadas uma vez antes de continuar com polling.
                espera_disponivel = false;
                if resultado != erro::CHAMADA {
                    supervisao.informar_erro(resultado, chamada::ESPERAR_IPC, 0);
                }
                sistema::ceder();
            }
        }
    }
}

fn processar_evento_vida(
    supervisao: &mut Supervisao,
    inscricao: &mut Option<u64>,
    evento: sistema::EventoSistema,
) {
    match evento {
        sistema::EventoSistema::Ipc(mensagem) => {
            if mensagem.origem == sistema::resolver(TERMINAL) {
                supervisao.processar(mensagem.codigo, mensagem.a);
            }
        }
        sistema::EventoSistema::Vida {
            referencia, tipo, ..
        } => {
            if Some(referencia) == *inscricao && tipo == contrato_sistema::evento_vida::INVALIDACAO
            {
                *inscricao = None;
            }
            // A política automática conserva consulta e backoff existentes.
            supervisao.consultar();
        }
        sistema::EventoSistema::Irq { .. } => {
            supervisao.informar_erro(erro::ESTADO, chamada::ESPERAR_EVENTO, 0);
        }
    }
}

fn desabilitar_vida(
    supervisao: &mut Supervisao,
    disponivel: &mut bool,
    inscricao: &mut Option<u64>,
    erro_recebido: u64,
) {
    if let Some(referencia) = inscricao.take() {
        let cancelamento = sistema::cancelar_vida(referencia);
        if cancelamento != 0 && cancelamento != erro::CHAMADA {
            supervisao.informar_erro(cancelamento, chamada::CANCELAR_VIDA, referencia);
        }
    }
    *disponivel = false;
    if erro_recebido != erro::CHAMADA {
        supervisao.informar_erro(erro_recebido, chamada::ESPERAR_EVENTO, 0);
    }
}

#[cfg(feature = "teste-vida")]
fn testar_vida() {
    use apoio_servicos::EventoSistema;
    use contrato_sistema::{evento_vida, prazo};
    fn falhar(etapa: u64) -> ! {
        notificar(0x771, etapa, 1, sistema::cpl() as u64, 0);
        sistema::sair()
    }
    fn etapa(n: u64) {
        notificar(0x771, n, 0, sistema::cpl() as u64, 0);
    }
    let terminal = sistema::resolver(TERMINAL);
    match sistema::receber_bloqueante() {
        Ok(m) if m.origem == terminal && m.codigo == 0x770 => {}
        _ => falhar(0),
    }
    let primeira = sistema::resolver(REDE);
    if sistema::inscrever_vida(primeira) != Ok(estado::EXECUTAVEL) {
        falhar(1);
    }
    if sistema::inscrever_vida(primeira).err() != Some(erro::ESTADO) {
        falhar(1);
    }
    etapa(1);
    let inicio = sistema::tiques();
    while sistema::tiques().wrapping_sub(inicio) < 1000 {
        sistema::ceder();
    }
    let mut mensagens = 0;
    let mut falha = false;
    for posicao in 0..17 {
        match sistema::esperar_evento(prazo::IMEDIATO, 0) {
            Ok(EventoSistema::Ipc(m))
                if m.origem == terminal
                    && m.codigo == 0x772
                    && m.a == mensagens + 1
                    && m.b == 0xabc
                    && m.c == 0
                    && m.d == 0 =>
            {
                mensagens += 1
            }
            Ok(EventoSistema::Vida {
                referencia,
                tipo,
                estado: valor,
            }) if referencia == primeira
                && tipo == evento_vida::ESTADO
                && valor == estado::FALHOU
                && posicao < 2
                && !falha =>
            {
                falha = true
            }
            _ => falhar(2),
        }
    }
    if mensagens != 16 || !falha {
        falhar(2);
    }
    if sistema::cancelar_vida(primeira) != 0
        || sistema::inscrever_vida(primeira) != Ok(estado::FALHOU)
        || sistema::esperar_evento(prazo::IMEDIATO, 0).err() != Some(erro::AUSENTE)
    {
        falhar(2);
    }
    etapa(2);
    let segunda = sistema::reiniciar(primeira);
    if !referencia_valida(segunda) || sistema::inscrever_vida(segunda).err() != Some(erro::RECURSOS)
    {
        falhar(3);
    }
    match sistema::esperar_evento(prazo::IMEDIATO, 0) {
        Ok(EventoSistema::Vida {
            referencia, tipo, ..
        }) if referencia == primeira && tipo == evento_vida::INVALIDACAO => {}
        _ => falhar(3),
    }
    if sistema::inscrever_vida(segunda) != Ok(estado::EXECUTAVEL) || sistema::encerrar(segunda) != 0
    {
        falhar(3);
    }
    match sistema::esperar_evento(prazo::IMEDIATO, 0) {
        Ok(EventoSistema::Vida {
            referencia,
            tipo,
            estado: valor,
        }) if referencia == segunda
            && tipo == evento_vida::ESTADO
            && valor == estado::FINALIZADO => {}
        _ => falhar(3),
    }
    if sistema::cancelar_vida(segunda) != 0 {
        falhar(3);
    }
    let terceira = sistema::reiniciar(segunda);
    if !referencia_valida(terceira)
        || sistema::esperar_evento(prazo::IMEDIATO, 0).err() != Some(erro::ESTADO)
        || sistema::inscrever_vida(primeira).err() != Some(erro::REFERENCIA_ANTIGA)
        || sistema::inscrever_vida(terceira) != Ok(estado::EXECUTAVEL)
        || sistema::esperar_evento(prazo::IMEDIATO, 0).err() != Some(erro::AUSENTE)
    {
        falhar(3);
    }
    etapa(3);
    match sistema::esperar_evento(prazo::SEM_PRAZO, 0) {
        Ok(EventoSistema::Vida {
            referencia,
            tipo,
            estado: valor,
        }) if referencia == terceira && tipo == evento_vida::ESTADO && valor == estado::FALHOU => {}
        _ => falhar(4),
    }
    etapa(4);
    let quarta = sistema::reiniciar(terceira);
    if !referencia_valida(quarta) {
        falhar(5);
    }
    match sistema::esperar_evento(prazo::IMEDIATO, 0) {
        Ok(EventoSistema::Vida {
            referencia, tipo, ..
        }) if referencia == terceira && tipo == evento_vida::INVALIDACAO => {}
        _ => falhar(5),
    }
    if sistema::inscrever_vida(quarta) != Ok(estado::EXECUTAVEL)
        || sistema::cancelar_vida(quarta) != 0
    {
        falhar(5);
    }
    etapa(5);
}

#[cfg(feature = "teste-prazo-idle")]
fn testar_prazo_idle() -> ! {
    use contrato_sistema::prazo;
    if sistema::receber_com_prazo(prazo::FINITO, 200).err() != Some(erro::TEMPO) {
        sistema::sair();
    }
    if sistema::enviar(sistema::resolver(TERMINAL), 0x732, 0xabc, 0xdef, 0x123, 0) != 0 {
        sistema::sair();
    }
    loop {
        sistema::ceder();
    }
}

#[cfg(feature = "teste-prazo")]
fn testar_prazo() {
    use contrato_sistema::prazo;
    let terminal = sistema::resolver(TERMINAL);
    if sistema::receber_com_prazo(prazo::IMEDIATO, 0).err() != Some(erro::AUSENTE)
        || sistema::receber_com_prazo(prazo::FINITO, 0).err() != Some(erro::CHAMADA)
        || sistema::receber_com_prazo(prazo::FINITO, 1 << 63).err() != Some(erro::CHAMADA)
    {
        sistema::sair();
    }
    if sistema::enviar(terminal, 0x722, 1, 0, 0, 0) != 0 {
        sistema::sair();
    }
    let Ok(primeira) = sistema::receber_com_prazo(prazo::FINITO, 5000) else {
        sistema::sair()
    };
    if primeira.origem != terminal
        || primeira.codigo != 0x721
        || primeira.a != 1
        || primeira.b != 0xabc
        || primeira.c != 0xdef
        || primeira.d != 0x123
    {
        sistema::sair();
    }
    if sistema::enviar(terminal, 0x722, 2, 0, 0, 0) != 0 {
        sistema::sair();
    }
    if sistema::receber_com_prazo(prazo::FINITO, 40).err() != Some(erro::TEMPO) {
        sistema::sair();
    }
    if sistema::enviar(terminal, 0x722, 3, 0, 0, 0) != 0 {
        sistema::sair();
    }
    let Ok(tardia) = sistema::receber_com_prazo(prazo::SEM_PRAZO, 0) else {
        sistema::sair()
    };
    if tardia.origem != terminal
        || tardia.codigo != 0x721
        || tardia.a != 2
        || tardia.b != 0xabc
        || tardia.c != 0xdef
        || tardia.d != 0x123
    {
        sistema::sair();
    }
    if sistema::enviar(terminal, 0x722, 4, 0, 0, 0) != 0 {
        sistema::sair();
    }
}

// A primeira mensagem permanece na fila após ESPERAR_IPC. As demais são
// enviadas depois de o recebimento bloquear e verificadas na ordem de chegada.
#[cfg(feature = "teste-receber-bloqueante")]
fn testar_receber_bloqueante() {
    let terminal = sistema::resolver(TERMINAL);
    if sistema::esperar_ipc() != 0 {
        sistema::sair();
    }
    for sequencia in 1..=3 {
        let Ok(mensagem) = sistema::receber_bloqueante() else {
            sistema::sair();
        };
        let codigo = if sequencia == 1 { erro::CHAMADA } else { 0x711 };
        if mensagem.origem != terminal
            || mensagem.codigo != codigo
            || mensagem.a != sequencia
            || mensagem.b != 0xabc
            || mensagem.c != 0xdef
            || mensagem.d != 0x123
        {
            sistema::sair();
        }
    }
    if sistema::enviar(terminal, 0x712, 1, 2, 3, 0xabc) != 0 {
        sistema::sair();
    }
}

#[panic_handler]
fn panico(_informacao: &core::panic::PanicInfo<'_>) -> ! {
    // O supervisor não pode garantir sua própria recuperação neste marco.
    sistema::sair()
}

// A espera indica disponibilidade; RECEBER retira a mensagem pela ABI 2.
#[cfg(feature = "teste-ipc-bloqueante")]
fn testar_ipc_bloqueante() {
    let terminal = sistema::resolver(TERMINAL);
    for sequencia in 1..=2 {
        if sistema::esperar_ipc() != 0 {
            sistema::sair();
        }
        let Some(mensagem) = sistema::receber() else {
            sistema::sair()
        };
        if mensagem.origem != terminal
            || mensagem.codigo != 0x701
            || mensagem.a != sequencia
            || mensagem.b != 0xabc
            || mensagem.c != 0
            || mensagem.d != 0
        {
            sistema::sair();
        }
    }
    if sistema::enviar(terminal, 0x702, 1, 2, 0xabc, 0) != 0 {
        sistema::sair();
    }
}
