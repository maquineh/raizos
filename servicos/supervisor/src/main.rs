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
        match sistema::estado_referencia(referencia) {
            estado::EXECUTAVEL => {
                self.espera = None;
                return;
            }
            estado::FALHOU | estado::FINALIZADO => {}
            _ => {
                self.espera = None;
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
                    self.reiniciar(referencia, true);
                }
                return;
            }
        }
        self.espera = Some(Espera {
            referencia,
            inicio: agora,
            atraso: ATRASOS[self.tentativas_automaticas],
        });
    }
}

#[no_mangle]
#[link_section = ".text.entrada"]
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
    let mut supervisao = Supervisao {
        automatica: false,
        reinicios: 0,
        tentativas_automaticas: 0,
        ultimo_erro: 0,
        espera: None,
    };
    notificar(
        evento::SUPERVISOR_PRONTO,
        0,
        MAX_INSTANCIAS_REDE,
        sistema::cpl() as u64,
        0,
    );
    loop {
        for _ in 0..16 {
            let Some(mensagem) = sistema::receber() else {
                break;
            };
            if mensagem.origem != sistema::resolver(TERMINAL) {
                continue;
            }
            supervisao.processar(mensagem.codigo, mensagem.a);
        }
        supervisao.consultar();
        sistema::ceder();
    }
}

#[panic_handler]
fn panico(_informacao: &core::panic::PanicInfo<'_>) -> ! {
    // O supervisor não pode garantir sua própria recuperação neste marco.
    sistema::sair()
}
