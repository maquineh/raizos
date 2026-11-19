# Execução RZ-015 — recebimento IPC bloqueante

Execução local sobre o commit `1ce0944` com alterações não commitadas. Os
arquivos de fonte e configuração ao final estão identificados em
[`fontes.sha256`](fontes.sha256). Após os builds, o parser do harness foi
corrigido para aceitar o prompt antes de um marcador completo do kernel; os
testes de host e o ensaio específico foram repetidos. O código do convidado
não mudou após os builds registrados.

## Ambiente e artefatos

Host Windows; rustc 1.99.1 (58cf8f9c3 2026-10-13), Cargo 1.99.1
(5f94df478 2026-08-27); QEMU 10.2.50 (v10.2.0-14086-g840320663e), TCG,
pc/i440fx, 1 CPU, 256 MiB, e1000 e OVMF. SHA-256 do firmware CODE:
`33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6db2a`;
VARS: `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e`.
O executável de Docker estava instalado, mas seu mecanismo não respondeu;
VirtualBox 7.0.26 estava instalado e não foi usado nesta execução. O QEMU é
a plataforma virtual de referência do projeto.

| Artefato local | SHA-256 |
| --- | --- |
| Imagem normal `saida/raiz-os.img` | `67e710fa298f3ae5f2c582120464d481c1c882a7b4db29b39f30956a84a1c79e` |
| EFI normal | `5be84816c6cff99b6c0188bc8b2aefd3ab068d405defbfa0d35306f6d42cb63f` |
| Imagem `saida/teste-receber-bloqueante/raiz-os.img` | `8ac21fdda7d15e87e7e6cce6fb905033cae4a757e05af95a345b3a599816cb58` |
| EFI do teste de recebimento | `77e05fbeb7fb387f93b7276208427d00ed3ec3d7a4aab0bbeb5e290a42937c0c` |
| Imagem `saida/teste-ipc-bloqueante/raiz-os.img` | `dc1b3129c12a0e92a92ae918430bca29756294032c748d1df5b7abcacd6c5c90` |
| EFI do teste IPC anterior | `1fe95a52639d2f032cd44b6f78b7a9788e227a292493e8e083e3a7ef164f795a` |
| Imagem `saida/teste-idle/raiz-os.img` | `8ead46211c42494212cac675e1e05cffbfb604c06dc9ce78106509425dda5b49` |
| EFI do teste idle | `180571e6605351fe784b30a78f675dc9e2d3755064bd4f5fcfc82b36c76ab0ed` |

Os hashes identificam os bytes locais, não autenticidade ou compilação
reproduzível. Logs de build: [normal](build-normal.log),
[recebimento](build-receber.log), [IPC anterior](build-ipc-anterior.log) e
[idle](build-idle.log).

## Comandos e resultados

| Comando | Resultado observado |
| --- | --- |
| `cargo fmt --all --check` | Passou. |
| `cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema` | Passou: 19 testes; contrato-boot tinha zero testes. [Log](testes-host-pacotes-final.log). |
| `cargo test -p nucleo --lib` | Passou: 17 testes. [Log](testes-host-nucleo-final.log). |
| `cargo raiz compilar` e `cargo raiz verificar-imagem` | Passaram; GPT/FAT32 e bytes EFI verificados. |
| `cargo raiz compilar --teste-receber-bloqueante` e `cargo raiz testar-receber-bloqueante` | Passaram na segunda tentativa; [serial](../teste-receber-bloqueante-1794722072073175400/serial.log) e [resultado](../teste-receber-bloqueante-1794722072073175400/resultado.txt). |
| `cargo raiz compilar --teste-ipc-bloqueante` e `cargo raiz testar-ipc-bloqueante` | Passaram; [serial](../teste-ipc-bloqueante-1794722098723292200/serial.log). |
| `cargo raiz compilar --teste-idle` e `cargo raiz testar-idle` | Passaram; [relatório](RELATORIO-TESTE-IDLE.md). |
| `cargo raiz testar` | Passou; [relatório](RELATORIO-SMOKE.md). |
| `cargo raiz testar-isolamento` | Passou em dois boots; [relatório](RELATORIO-ISOLAMENTO.md). |
| `cargo raiz testar-recuperacao` | Passou em dois boots; [relatório](RELATORIO-RECUPERACAO.md). |

O cenário específico usou terminal e supervisor em CPL 3. Primeiro, uma
mensagem já estava na fila; seu código era igual a `erro::CHAMADA`, demonstrando
que ele não se confunde com o status da syscall. Depois, o supervisor bloqueou
com a caixa vazia, permaneceu sem conclusão durante o intervalo observado e
recebeu mensagens enviadas pelo terminal, que conferiu a resposta. O serviço
verificou referência de origem, valores e ordem. A execução não garante que
as duas mensagens posteriores tenham estado simultaneamente na fila.

## Tentativa anterior e limites

A [primeira tentativa](../teste-receber-bloqueante-1794721941063068900/serial.log)
terminou com timeout porque o verificador exigia o marcador em linha isolada,
mas o prompt precedeu o marcador real na mesma linha. O parser passou a aceitar
esse prefixo específico, mantendo linha completa e ordem; a segunda tentativa
passou. A falha e o log foram preservados.

A purga entre despertar e seleção, incluindo rearmar após esvaziamento, foi
exercitada no modelo compartilhado pelo núcleo em teste de host, não em VM.
O novo serviço com kernel ABI 2 antigo não foi combinado em VM para testar o
fallback. A operação não oferece prazo, IRQ delegada ou cancelamento remoto; a
versão 0.3 completa permanece pendente. O supervisor no modo automático, a rede
e o terminal continuam usando polling. As evidências históricas foram
restauradas em seus caminhos originais após as regressões.
