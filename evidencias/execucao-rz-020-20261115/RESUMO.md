# Execução RZ-020 — entrega e reconhecimento de IRQ4

Execução local sobre o commit `1ce0944`, com alterações não commitadas. As
[fontes](fontes.sha256) e os [artefatos](artefatos.sha256) identificam os
arquivos da execução final. Os hashes identificam bytes, sem demonstrar
autenticidade ou compilação reproduzível. A ABI permanece na versão 2; a
[ADR 010](../../docs/ADR-010-NOTIFICACAO-IRQ4.md) descreve o contrato e os limites.

## Ambiente

Windows; rustc 1.99.1 (58cf8f9c3 2026-10-13), Cargo 1.99.1
(5f94df478 2026-08-27); QEMU 10.2.50 (v10.2.0-14086-g840320663e), TCG,
pc/i440fx, 1 CPU, 256 MiB, e1000 e OVMF. SHA-256 do firmware CODE:
`33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6db2a`;
VARS: `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e`.
O [log de ambiente](ambiente.log) registra as versões e hashes observados.

## Comandos e resultados

| Comando | Resultado observado |
| --- | --- |
| `cargo fmt --all --check` | Passou. [Log](formatacao.log). |
| `cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema` | Passou: 23 testes; contrato-boot tinha zero testes. [Log](testes-host-pacotes.log). |
| `cargo test -p nucleo --lib` | Passou: 31 testes. [Log](testes-host-nucleo.log). |
| `cargo raiz compilar` e `cargo raiz verificar-imagem` | Passaram na imagem normal final. [Build](build-normal.log), [verificação](verificar-imagem.log). |
| `cargo raiz compilar --teste-irq` e `cargo raiz testar-irq` | Passaram em imagem isolada. [Build](build-irq-final.log), [comando](comando-irq-final.log), [serial](../teste-irq-1794725963506321900/serial.log), [resultado](../teste-irq-1794725963506321900/resultado.txt). |
| `cargo raiz testar` | Passou. [Relatório](RELATORIO-SMOKE.md). |
| `cargo raiz testar-isolamento` | Passou em dois boots. [Relatório](RELATORIO-ISOLAMENTO.md). |
| `cargo raiz testar-recuperacao` | Passou em dois boots. [Relatório](RELATORIO-RECUPERACAO.md). |
| `cargo raiz compilar --teste-quota` e `cargo raiz testar-quota` | Passaram. [Serial](../teste-quota-1794726070029937300/serial.log), [resultado](../teste-quota-1794726070029937300/resultado.txt). |
| `cargo raiz compilar --teste-vida` e `cargo raiz testar-vida` | Passaram. [Serial](../teste-vida-1794726091503292900/serial.log), [resultado](../teste-vida-1794726091503292900/resultado.txt). |
| `cargo raiz compilar --teste-prazo-idle` e `cargo raiz testar-prazo-idle` | Passaram. [Serial](../teste-prazo-idle-1794726114872668800/serial.log), [resultado](../teste-prazo-idle-1794726114872668800/resultado.txt). |

## Cobertura e limites

A imagem opt-in associou a IRQ4 exclusiva ao terminal em CPL 3. O supervisor
teve associação, espera e confirmação recusadas; o terminal verificou recusa de
associação duplicada e de sequências incorreta e já confirmada. Com os três
serviços bloqueados, o host enviou A pela UART e o kernel registrou IRQ4 no
contexto ocioso CPL 0. O terminal drenou A e o host enviou B antes do rearme.

Com B ainda pendente e sem confirmação, o ensaio amostrou o bit de máscara do
PIC e um contador saturante de entradas no handler; após pelo menos 50 tiques,
o bit continuava mascarado e o contador não havia mudado. A máscara foi
observada nos dois extremos, não continuamente. O avanço dos tiques demonstrou
progresso do temporizador nesse intervalo. Depois da confirmação, o terminal
verificou novamente a UART, leu B e tornou a bloquear; C provocou nova entrega
com sequência distinta. O ensaio aceita avisos sem byte, que podem resultar de
uma borda anteriormente retida no PIC. As medições de contador e máscara usam
uma chamada privada e instrumentação presentes apenas na imagem de teste.

O modelo host exercitou coalescência, sequência, recusa e exaustão; isso não é
uma medição de coalescência física. O intervalo observado em QEMU não demonstra
ausência de tempestade sob carga arbitrária. Não foram demonstradas em VM a
revogação de IRQ por falha real do terminal nem a interação com FIFO IPC cheia.
O terminal normal ainda usa polling; o piloto de produção RZ-021 e a integração
geral RZ-022 permanecem propostos. Não se declara suporte a IRQ compartilhada,
hardware físico, SMP, perda zero sob carga ilimitada ou conclusão do marco 0.3.

## Tentativas e preservação

O [primeiro build instrumentado](build-irq-tentativa-1.log) falhou por E0499 no
binário do kernel; a correção foi compilada. Um [primeiro ensaio aprovado](../teste-irq-1794725671682654900/resultado.txt)
cobriu entrega e rearme sem demonstrar IRQ no idle; o roteiro foi ampliado. O
[ensaio ampliado anterior](../teste-irq-1794725802752374300/resultado.txt) e os
[cinco boots normais anteriores](antes-correcao-static/) passaram, mas os builds
emitiam cinco avisos sobre referências a `static mut`. Os acessos foram ajustados
sob o invariante de CPU única e IF=0; os builds e testes finais foram repetidos.
Os [hashes das fontes](fontes-antes-correcao.sha256) e dos
[artefatos anteriores](artefatos-antes-correcao.sha256) foram preservados, bem
como os relatórios da tentativa anterior. Os relatórios históricos de outras
execuções foram restaurados após as regressões.
