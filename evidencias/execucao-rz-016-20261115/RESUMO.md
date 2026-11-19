# Execução RZ-016 — prazos de recepção IPC

Execução local sobre o commit `1ce0944` com alterações não commitadas. Os
arquivos de fonte e configuração utilizados estão identificados em
[`fontes.sha256`](fontes.sha256); as imagens e seus carregadores estão em
[`artefatos.sha256`](artefatos.sha256). A ABI permanece na versão 2, com a
extensão aditiva `RECEBER_COM_PRAZO`, documentada na
[ADR 006](../../docs/ADR-006-PRAZOS-IPC.md).

## Ambiente

Windows; rustc 1.99.1 (58cf8f9c3 2026-10-13), Cargo 1.99.1
(5f94df478 2026-08-27); QEMU 10.2.50 (v10.2.0-14086-g840320663e), TCG,
pc/i440fx, 1 CPU, 256 MiB, e1000 e OVMF. SHA-256 do firmware CODE:
`33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6db2a`;
VARS: `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e`.
As imagens em `saida/` são locais; seus hashes identificam os bytes testados,
sem demonstrar autenticidade ou compilação reproduzível.

## Comandos e resultados

| Comando | Resultado observado |
| --- | --- |
| `cargo fmt --all --check` | Passou. |
| `cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema` | Passou: 20 testes; contrato-boot tinha zero testes. [Log](testes-host-pacotes.log). |
| `cargo test -p nucleo --lib` | Passou: 21 testes. [Log](testes-host-nucleo.log). |
| `cargo raiz compilar` e `cargo raiz verificar-imagem` | Passaram; GPT/FAT32 e bytes EFI verificados. [Build](build-normal.log), [verificação](verificar-imagem.log). |
| `cargo raiz compilar --teste-prazo` e `cargo raiz testar-prazo` | Passaram em imagem isolada. [Serial](../teste-prazo-1794722646418998500/serial.log), [resultado](../teste-prazo-1794722646418998500/resultado.txt). |
| `cargo raiz compilar --teste-prazo-idle` e `cargo raiz testar-prazo-idle` | Passaram em imagem isolada. [Serial](../teste-prazo-idle-1794722659073305000/serial.log), [resultado](../teste-prazo-idle-1794722659073305000/resultado.txt). |
| `cargo raiz testar` | Passou. [Relatório](RELATORIO-SMOKE.md). |
| `cargo raiz testar-isolamento` | Passou em dois boots. [Relatório](RELATORIO-ISOLAMENTO.md). |
| `cargo raiz testar-recuperacao` | Passou em dois boots. [Relatório](RELATORIO-RECUPERACAO.md). |
| Ensaios anteriores de recebimento bloqueante, IPC e idle | Imagens reconstruídas e testes passaram. Logs de comando: [recebimento](comando-receber-anterior.log), [IPC](comando-ipc-anterior.log), [idle](comando-idle-anterior.log). |

No primeiro ensaio de prazo, os serviços em CPL 3 verificaram ausência imediata,
parâmetros inválidos, mensagem recebida antes de prazo finito, timeout e
recebimento posterior de mensagem enviada após o timeout. No segundo, terminal,
rede e supervisor bloquearam; IRQ0 ocorreu durante idle em CPL 0, o prazo do
supervisor expirou, e ele enviou IPC ao terminal, que validou origem e dados em
CPL 3. O teste não provoca o despertar artificialmente.

## Limites

A prioridade de timeout em empate, a conclusão única, os limites e rollover do
contador, e a substituição do estado de instância foram exercitados em testes
de host do modelo usado pelo núcleo; esses interleavings não foram reproduzidos
em VM. O tempo é contado em tiques de IRQ0 observados e não constitui garantia
de tempo físico ou tempo real. Uma mensagem admitida antes do vencimento pode
permanecer na fila se o prazo vencer antes de a recepção ser concluída.

A combinação de serviço novo e kernel ABI 2 anterior não foi ensaiada em VM.
O supervisor normal ainda não usa a nova chamada com prazo; o modo automático,
o terminal e a rede mantêm polling. Espera multiplexada, cancelamento remoto,
notificações de IRQ e o marco 0.3 completo seguem pendentes. As evidências
históricas foram preservadas e restauradas após as regressões.
