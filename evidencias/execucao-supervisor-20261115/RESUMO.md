# Execução RZ-014 — supervisor aguardando IPC

Execução local sobre o commit `1ce0944` com alterações não commitadas. Os
arquivos efetivos de fonte e configuração são identificados por
[`fontes.sha256`](fontes.sha256). A ABI continua em 2 e inclui a extensão
aditiva `ESPERAR_IPC`, conforme a [ADR 004](../../docs/ADR-004-ESPERA-IPC-ABI-2.md).

## Ambiente e artefatos

Host Windows; rustc 1.99.1 (58cf8f9c3 2026-10-13), Cargo 1.99.1
(5f94df478 2026-08-27); QEMU 10.2.50 (v10.2.0-14086-g840320663e), TCG,
pc/i440fx, 1 CPU, 256 MiB, e1000 e OVMF. SHA-256 de CODE:
`33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6db2a`;
VARS: `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e`.

| Artefato local | SHA-256 |
| --- | --- |
| Imagem normal `saida/raiz-os.img` | `36713b1338468114ac14ed256a9a5c5fb501e4800c0864ab4a40f2d483259327` |
| EFI normal | `6129cb63686b7465c77c17401d32c9da628b0aefe05abc99700786d64ac3ae15` |
| Imagem `saida/teste-ipc-bloqueante/raiz-os.img` | `d1e64e94e7e0c47506356ed7f7b5707d9cf9298b3820af351c9cee7172674e01` |
| EFI do teste IPC | `858a290d4062d802a02e8fa79b1868f5b0b2467f62073a6cb6f9d367714f968b` |
| Imagem `saida/teste-idle/raiz-os.img` | `2f61dbc777b55b30ee5adcd5f172d4188036d6bcfd908be7e042fbf69424e3de` |
| EFI do teste idle | `d8e89475404f8108ed57b3d199c2e4de3a52664d27c6c738b439024aefac8625` |

Os hashes identificam os bytes dos artefatos locais; não demonstram
autenticidade ou compilação reproduzível. Logs de build:
[normal](build-normal.log), [IPC](build-ipc.log) e [idle](build-idle.log).

## Comandos e resultados

| Comando | Resultado observado |
| --- | --- |
| `cargo fmt --all --check` | Passou. |
| `cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema` | Passou: 18 testes; contrato-boot tinha zero testes. [Log](testes-host-pacotes.log). |
| `cargo test -p nucleo --lib` | Passou: 15 testes. [Log](testes-host-nucleo.log). |
| `cargo raiz compilar` e `cargo raiz verificar-imagem` | Passaram; GPT/FAT32 e bytes EFI verificados. |
| `cargo raiz compilar --teste-ipc-bloqueante` e `cargo raiz testar-ipc-bloqueante` | Passaram; [serial e resultado](../teste-ipc-bloqueante-1794721434165104700/). |
| `cargo raiz compilar --teste-idle` e `cargo raiz testar-idle` | Passaram; [relatório](RELATORIO-TESTE-IDLE.md). |
| `cargo raiz testar` | Passou; [relatório](RELATORIO-SMOKE.md). |
| `cargo raiz testar-isolamento` | Passou em dois boots; [relatório](RELATORIO-ISOLAMENTO.md). |
| `cargo raiz testar-recuperacao` | Passou em dois boots; [relatório](RELATORIO-RECUPERACAO.md). |

O ensaio IPC instrumentado observou o supervisor bloqueado, uma admissão IPC
do terminal, o despertar e uma resposta em CPL 3 que valida origem, dados e
ordem de duas mensagens. Depois observou novo bloqueio no laço manual real,
despertar pelo comando `supervisao` e resposta de estado. O cenário idle usa
uma liberação artificial pela terceira IRQ0. As regressões usaram a imagem normal.

## Limites

Em modo manual o supervisor espera por atividade IPC; no automático continua
consultando estado e prazos. Terminal e rede continuam em polling. A espera
indica disponibilidade, não retira nem reserva mensagem: uma purga por
revogação pode esvaziar a fila antes de `RECEBER`. Prazos, espera multiplexada,
notificações de IRQ e proteção contra eventos assíncronos antigos permanecem
futuros. A compatibilidade de um supervisor novo com kernel ABI 2 antigo foi
analisada pelo retorno `CHAMADA` e pelo fallback para cessão, mas essa mistura
de binários não foi ensaiada em VM.

As evidências históricas nos caminhos originais foram preservadas e restauradas;
os relatórios desta execução estão nesta pasta.
