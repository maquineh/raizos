# Execução RZ-014 — despertar por IPC

Esta execução parte do commit `1ce0944` com alterações locais não commitadas.
O [manifesto de fontes](fontes.sha256) registra hashes da árvore ao final da
execução. Depois dos builds e das regressões, o harness foi alterado apenas
para exigir a ordem dos marcadores no ensaio IPC; esse ensaio e os testes de
host foram então repetidos. O código do convidado não mudou após os builds.

## Ambiente e imagens

Windows; rustc 1.99.1 (58cf8f9c3 2026-10-13), Cargo 1.99.1
(5f94df478 2026-08-27); QEMU 10.2.50 (v10.2.0-14086-g840320663e), TCG,
máquina pc/i440fx, 1 CPU, 256 MiB, e1000 e OVMF. SHA-256 do firmware CODE:
`33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6db2a`;
VARS: `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e`.

| Artefato local | SHA-256 |
| --- | --- |
| Imagem normal `saida/raiz-os.img` | `e2ed1f78ad78e41c06990b10ec82a24358b88ec4fb9c18d1e206ba14e3d9b357` |
| EFI normal | `ab039690d997aa6dd1ddd849a9769d6b993cd14235005a00308f9543727d4a12` |
| Imagem `saida/teste-ipc-bloqueante/raiz-os.img` | `f8ee3e3a6dc1725ae63a05fd0ef8cafe1d96a238416c325053d69b0788b2f328` |
| EFI de teste IPC | `a55ef31f03c9290db83027d7bff5465a51b4d964b17c47d40c4a919bc69ad7ae` |
| Imagem `saida/teste-idle/raiz-os.img` | `7b5460084d2e45e55065c5cbafba565bc3eb39980c770c89adb98a5799da6367` |
| EFI de teste idle | `2d06185933b382deb418aeb41604673d571b454049022d5326ea6916ac39d05c` |

Os artefatos em `saida/` são locais e não integram o Git. Esses identificadores
associam os registros aos bytes executados; não demonstram autenticidade nem
compilação reproduzível. Logs: [build normal](build-normal.log),
[build IPC](build-teste-ipc.log) e [build idle](build-teste-idle.log).

## Resultados

| Comando | Resultado observado |
| --- | --- |
| `cargo fmt --all --check` | Passou. |
| `cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema` | Passou: 18 testes; contrato-boot tinha zero testes. [Log](testes-host-pacotes-final.log). |
| `cargo test -p nucleo --lib` | Passou: 15 testes. [Log](testes-host-nucleo-final.log). |
| `cargo raiz compilar` e `cargo raiz verificar-imagem` | Passaram; GPT/FAT32 e bytes EFI foram verificados. |
| `cargo raiz compilar --teste-ipc-bloqueante` e `cargo raiz testar-ipc-bloqueante` | Passaram em imagem separada. [Log serial do ensaio final](../teste-ipc-bloqueante-1794721124835151200/serial.log). |
| `cargo raiz compilar --teste-idle` e `cargo raiz testar-idle` | Passaram em imagem separada. [Relatório](RELATORIO-TESTE-IDLE.md). |
| `cargo raiz testar` | Passou. [Relatório](RELATORIO-SMOKE.md). |
| `cargo raiz testar-isolamento` | Passou nos dois boots. [Relatório](RELATORIO-ISOLAMENTO.md). |
| `cargo raiz testar-recuperacao` | Passou nos dois boots. [Relatório](RELATORIO-RECUPERACAO.md). |

No ensaio IPC, o supervisor real em CPL 3 registrou espera na caixa vazia.
O harness observou a espera sem despertar por 500 ms antes de enviar um comando
ao terminal. O terminal enviou duas mensagens autorizadas; a primeira admissão
despertou o supervisor, que conferiu origem, conteúdo e ordem e respondeu ao
terminal. O oráculo final exige despertar antes da resposta validada, em linhas
completas, e rejeita marcadores de falha. Uma [primeira execução aprovada](../teste-ipc-bloqueante-1794720958567407100/serial.log)
foi preservada antes do reforço desse oráculo.

O ensaio usa chamada privada apenas na imagem de teste. Ela sinaliza
disponibilidade e o serviço usa `RECEBER` da ABI 2 para consumir; uma revogação
pode purgar a mensagem antes da leitura. Os serviços da imagem normal ainda
consultam suas fontes em polling. O teste não comprova espera multiplexada,
prazos, interrupções delegadas, cancelamento de espera por pares ou proteção
contra eventos assíncronos atrasados após troca de geração. O registro atual de
espera é local à instância e não é um token de geração para timers futuros.

As evidências históricas foram copiadas antes das regressões e restauradas em
seus caminhos originais. Os relatórios desta execução permanecem nesta pasta.
