# Execução RZ-018 — notificações de ciclo de vida

Execução local sobre o commit `1ce0944`, com alterações não commitadas. As
[fontes](fontes.sha256) e os [artefatos](artefatos.sha256) identificam os
arquivos usados na execução final. Os hashes identificam bytes, sem demonstrar
autenticidade ou compilação reproduzível. A ABI permanece na versão 2; o
contrato e seus limites estão na [ADR 008](../../docs/ADR-008-NOTIFICACOES-VIDA.md).

## Ambiente

Windows; rustc 1.99.1 (58cf8f9c3 2026-10-13), Cargo 1.99.1
(5f94df478 2026-08-27); QEMU 10.2.50 (v10.2.0-14086-g840320663e), TCG,
pc/i440fx, 1 CPU, 256 MiB, e1000 e OVMF. SHA-256 do firmware CODE:
`33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6db2a`;
VARS: `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e`.

## Comandos e resultados

| Comando | Resultado observado |
| --- | --- |
| `cargo fmt --all --check` | Passou. [Log](formatacao.log). |
| `cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema` | Passou: 23 testes; contrato-boot tinha zero. [Log](testes-host-pacotes.log). |
| `cargo test -p nucleo --lib` | Passou: 27 testes. [Log](testes-host-nucleo.log). |
| `cargo raiz compilar` e `cargo raiz verificar-imagem` | Passaram na imagem normal final. [Build](build-normal.log), [verificação](verificar-imagem.log). |
| `cargo raiz compilar --teste-vida` e `cargo raiz testar-vida` | Passaram. [Comando](comando-vida.log), [serial](../teste-vida-1794724542137348500/serial.log), [resultado](../teste-vida-1794724542137348500/resultado.txt). |
| `cargo raiz testar` | Passou na imagem normal final. [Relatório](RELATORIO-SMOKE.md). |
| `cargo raiz testar-isolamento` | Passou em dois boots da imagem normal final. [Relatório](RELATORIO-ISOLAMENTO.md). |
| `cargo raiz testar-recuperacao` | Passou em dois boots da imagem normal final. [Relatório](RELATORIO-RECUPERACAO.md). |
| `cargo raiz compilar --teste-ciclo-espera` e `cargo raiz testar-ciclo-espera` | Passaram. [Serial](../teste-ciclo-espera-1794724583709320900/serial.log), [resultado](../teste-ciclo-espera-1794724583709320900/resultado.txt). |
| `cargo raiz compilar --teste-observacao` e `cargo raiz testar-observacao` | Passaram. [Serial](../teste-observacao-1794724618090843000/serial.log), [resultado](../teste-observacao-1794724618090843000/resultado.txt). |
| Ensaios anteriores de prazo, prazo em idle e recepção bloqueante | Imagens reconstruídas e testes passaram. [Prazo](comando-prazo.log), [prazo em idle](comando-prazo-idle.log), [recepção](comando-receber-bloqueante.log). |

## Cenários e limites

O terminal teve inscrição negada. O supervisor inscreveu-se na rede por
referência exata; com sua FIFO de 16 mensagens cheia, recebeu a notificação de
falha sem depender de espaço nela. O ensaio verificou cancelamento, estado
inicial, invalidação da geração antiga, limite do slot, encerramento normal da
segunda geração, ausência de evento obsoleto após cancelamento e reinscrição,
e despertar por falha da terceira geração. Após o reinício para a quarta
geração, os testes de conectividade ping e UDP passaram.

Há um slot por observador autorizado, atualmente supervisor→rede; eventos de
estado são coalescidos e a invalidação terminal ocupa o slot até ser consumida
ou cancelada. A escolha entre IPC e vida, as regras de coalescência e o desempate
com timeout estão descritos na ADR e foram exercitados no host. A espera geral
de IRQ e quotas abrangentes não fazem parte deste recorte. O supervisor manual
usa a espera IPC/vida; o automático continua consultando estado e prazos. A
combinação de kernel ABI 2 antigo com serviços novos não foi testada em VM.
Não há, aqui, validação de hardware físico ou promoção do marco 0.3.

## Tentativas preservadas

A [primeira tentativa](../teste-vida-1794724330187689900/resultado.txt) falhou
na etapa 3: a fixture esperava `AUSENTE` após cancelar a inscrição, enquanto o
contrato retorna `ESTADO` sem inscrição ativa. A fixture foi corrigida e passou
a verificar `AUSENTE` após inscrever a nova geração; o [serial da falha](../teste-vida-1794724330187689900/serial.log)
e os logs do [primeiro build](build-vida-tentativa-1.log) e
[comando](comando-vida-tentativa-1.log) foram
preservados. A imagem normal anterior também passou nas regressões; seus
[relatórios e hashes](normal-anterior/) foram preservados. Como a correção da
fixture alterou a fonte, a imagem normal foi reconstruída e os cinco boots de
regressão foram repetidos. Os relatórios históricos de outras execuções foram
restaurados após as verificações.
