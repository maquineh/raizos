# Execução RZ-021 — piloto UART COM1 com IRQ4

Execução local sobre o commit `1ce0944`, com alterações não commitadas. As
[fontes](fontes.sha256) e os [artefatos](artefatos.sha256) identificam os
arquivos usados na execução final. Os hashes identificam bytes, sem demonstrar
autenticidade ou compilação reproduzível. A ABI permanece na versão 2; a
[ADR 011](../../docs/ADR-011-TERMINAL-UART.md) registra o laço do terminal.

## Ambiente

Windows; rustc 1.99.1 (58cf8f9c3 2026-10-13), Cargo 1.99.1
(5f94df478 2026-08-27); QEMU 10.2.50 (v10.2.0-14086-g840320663e), TCG,
pc/i440fx, 1 CPU, 256 MiB, e1000 e OVMF. SHA-256 do firmware CODE:
`33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6db2a`;
VARS: `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e`.
O [log de ambiente](ambiente.log) registra versões e hashes observados.

## Comandos e resultados

| Comando | Resultado observado |
| --- | --- |
| `cargo fmt --all --check` | Passou. [Log](formatacao.log). |
| `cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema` | Passou: 23 testes; contrato-boot tinha zero. [Log](testes-host-pacotes.log). |
| `cargo test -p nucleo --lib` | Passou: 31 testes. [Log](testes-host-nucleo.log). |
| `cargo raiz compilar` e `cargo raiz verificar-imagem` | Passaram na imagem normal final. [Build](build-normal.log), [verificação](verificar-imagem.log). |
| `cargo raiz testar-uart` | Passou na imagem normal. [Comando](comando-uart-normal.log), [serial](../teste-uart-1794726920528122700/serial.log), [resultado](../teste-uart-1794726920528122700/resultado.txt). |
| `cargo raiz compilar --teste-uart` e `cargo raiz testar-uart --instrumentado` | Passaram em imagem isolada. [Build](build-uart-final.log), [comando](comando-uart-instrumentado.log), [serial](../teste-uart-1794726895277854900/serial.log), [resultado](../teste-uart-1794726895277854900/resultado.txt). |
| `cargo raiz testar` | Passou. [Relatório](RELATORIO-SMOKE.md). |
| `cargo raiz testar-isolamento` | Passou em dois boots. [Relatório](RELATORIO-ISOLAMENTO.md). |
| `cargo raiz testar-recuperacao` | Passou em dois boots. [Relatório](RELATORIO-RECUPERACAO.md). |
| Ensaios reconstruídos de IRQ, quota, vida e prazo no idle | Passaram. [IRQ](../teste-irq-1794727015885039400/resultado.txt), [quota](../teste-quota-1794727028551910900/resultado.txt), [vida](../teste-vida-1794727051609060500/resultado.txt), [prazo/idle](../teste-prazo-idle-1794727076056907600/resultado.txt). |
| `./evidencias/execucao-rz-021-20261115/testar-ps2.ps1` no PowerShell | Passou na imagem normal: `sobre` digitado por PS/2 emulado via monitor do QEMU e resposta distinta observada. [Script](testar-ps2.ps1), [hash](script-ps2.sha256), [serial](ps2-20261115-072013-2301210/serial.log), [resultado](ps2-20261115-072013-2301210/resultado.txt). Requer `RAIZ_QEMU`, `RAIZ_OVMF` e `RAIZ_OVMF_VARS`; `RAIZ_QEMU_DADOS` é opcional. |

## Cobertura e limites

Na imagem instrumentada, o núcleo registrou que uma espera já bloqueada do
laço real terminou com uma IRQ. O host enviou `s`, aguardou a janela de teste e
enviou `o` antes da confirmação; a fixture observou dados pendentes tanto antes
da confirmação quanto na rechecagem posterior, e o comando `sobre` recebeu a
resposta completa. O hook e os marcadores são opt-in e alteram a temporização;
o primeiro marcador de bloqueio não garante que aquela mesma espera ainda
estivesse ativa quando o host enviou o byte. O evento `DESPERTA_IRQ` identifica
a conclusão de uma espera previamente bloqueada, não uma conclusão por timeout.

O teste funcional da imagem normal enviou caracteres isolados a intervalos de
50 ms e duas sequências em blocos de quatro bytes separados por 50 ms. Exigiu
respostas completas em linhas distintas do eco, e rejeitou diagnósticos de erro
UART e de fallback. No recorte ensaiado não houve perda observada; a FIFO pode
transbordar sob outras cargas, e flags LSR não contam bytes perdidos. A negação
a outro serviço e o rearme foram novamente exercitados no ensaio de IRQ anterior.
O ensaio PS/2 usa o teclado emulado do QEMU, não hardware físico.

O terminal limita o trabalho serial e alterna prioridade com PS/2; a espera tem
prazo de um tique para o teclado. RX não é consultada a cada timeout no modo IRQ;
após armar ou confirmar, a verificação de estado é necessária para o protocolo
de rearme. Em erro, há fallback para polling e desativação da fonte UART; a
associação já criada pode permanecer no núcleo. A combinação com kernel antigo
não foi ensaiada em VM. Não se declara perda zero sob carga arbitrária, tempo
real, suporte físico, IRQ compartilhada ou conclusão de RZ-022 e do marco 0.3.

## Tentativas e preservação

O [primeiro teste instrumentado](../teste-uart-1794726671224390100/resultado.txt)
falhou porque o harness aguardava um marcador diferente do emitido pelo terminal;
o oráculo foi corrigido. Um [ensaio seguinte](../teste-uart-1794726718052693100/resultado.txt)
passou, mas não distinguia com rigor chegada antes da confirmação e depois dela.
O hook e o oráculo foram reforçados e o ensaio final repetido. As fontes e
imagens anteriores estão identificadas pelos [hashes das fontes](fontes-antes-oraculo.sha256)
e [artefatos](artefatos-antes-oraculo.sha256); os [cinco boots anteriores](antes-oraculo-janela/)
foram preservados e repetidos na imagem final.

A [primeira tentativa PS/2](ps2-20261115-071901-4124843/resultado.txt) falhou
antes da prontidão por compartilhamento do arquivo serial no Windows; o script
foi corrigido e repetido. Os relatórios históricos de outras execuções foram
restaurados após as regressões. As cópias locais de variáveis OVMF permanecem em
`target/`, não nas evidências.
