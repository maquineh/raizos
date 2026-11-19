# Execução RZ-017 — cancelamento e ciclo de vida

Execução local sobre o commit `1ce0944` com alterações não commitadas. Os
arquivos de fonte e configuração efetivos estão identificados em
[`fontes.sha256`](fontes.sha256); imagens e carregadores estão em
[`artefatos.sha256`](artefatos.sha256). A ABI permanece na versão 2; a
observação pontual está documentada na
[ADR 007](../../docs/ADR-007-ESPERA-E-CICLO-DE-VIDA.md).

## Ambiente

Windows; rustc 1.99.1 (58cf8f9c3 2026-10-13), Cargo 1.99.1
(5f94df478 2026-08-27); QEMU 10.2.50 (v10.2.0-14086-g840320663e), TCG,
pc/i440fx, 1 CPU, 256 MiB, e1000 e OVMF. SHA-256 do firmware CODE:
`33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6db2a`;
VARS: `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e`.
As imagens em `saida/` são locais. Seus hashes identificam os bytes testados,
sem demonstrar autenticidade nem compilação reproduzível.

## Comandos e resultados

| Comando | Resultado observado |
| --- | --- |
| `cargo fmt --all --check` | Passou. |
| `cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema` | Passou: 22 testes; contrato-boot tinha zero testes. [Log](testes-host-pacotes-final.log). |
| `cargo test -p nucleo --lib` | Passou: 23 testes. [Log](testes-host-nucleo-final.log). |
| `cargo raiz compilar` e `cargo raiz verificar-imagem` | Passaram; GPT/FAT32 e bytes EFI verificados. [Build](build-normal.log), [verificação](verificar-imagem.log). |
| `cargo raiz compilar --teste-ciclo-espera` e `cargo raiz testar-ciclo-espera` | Passaram em imagem isolada. [Serial](../teste-ciclo-espera-1794723488638996400/serial.log), [resultado](../teste-ciclo-espera-1794723488638996400/resultado.txt). |
| `cargo raiz compilar --teste-observacao` e `cargo raiz testar-observacao` | Passaram em imagem isolada. [Serial](../teste-observacao-1794723464859922300/serial.log), [resultado](../teste-observacao-1794723464859922300/resultado.txt). |
| `cargo raiz testar` | Passou. [Relatório](RELATORIO-SMOKE.md). |
| `cargo raiz testar-isolamento` | Passou em dois boots. [Relatório](RELATORIO-ISOLAMENTO.md). |
| `cargo raiz testar-recuperacao` | Passou em dois boots. [Relatório](RELATORIO-RECUPERACAO.md). |
| Ensaios anteriores de prazo, prazo em idle e recepção bloqueante | Imagens reconstruídas e testes passaram. Logs de comando: [prazo](comando-prazo.log), [prazo em idle](comando-prazo-idle.log), [recepção](comando-receber.log). |

## Cenários de ciclo de vida

No ensaio de ciclo, a primeira rede foi encerrada enquanto aguardava e seu
registro foi cancelado. A segunda ficou bloqueada além do tique em que o prazo
da primeira venceria, sem ser despertada por ele; depois uma mensagem válida a
liberou. A terceira foi substituída enquanto bloqueada. Na quarta, a recusa de
reinício por quota ocorreu antes do timeout, que chegou exatamente no prazo
registrado para essa instância; a rede então inicializou e respondeu a ping e
UDP. As arenas antigas permaneceram em quarentena.

No ensaio de observação, o terminal recebeu `PERMISSAO`; o supervisor aguardou
uma referência exata da rede, que executou UD2 provocado pelo teste. O supervisor
recebeu `ALVO_INDISPONIVEL`; foram ainda verificados o alvo já falho, a referência
antiga após reinício e a retomada de ping e UDP. A observação é pontual e termina
com a chamada; não cria inscrição persistente de ciclo de vida.

## Tentativas e limites

O [primeiro build instrumentado](build-ciclo-tentativa-1.log) compilou com um
aviso sobre referência a `static mut`. O trecho de diagnóstico foi corrigido e
o build final passou sem esse aviso. A primeira imagem normal também passou
pelas regressões, mas a reconstrução posterior produziu bytes diferentes;
por isso os cinco boots foram repetidos na imagem final. Os
[registros da primeira imagem normal](primeira-imagem-normal/) foram preservados;
seu SHA-256 era `e6dd616b31b1aea8dedc5d2b7e3abe9485061619a5cc9cd6a0c6d77c5ee957b0`
e o de seu EFI era `1510a3a3a6bbb09cca6ed50700bd94e02761b77dcc432b46ca17414505aa41cd`.

Falha de preparação de imagem simulada no auxiliar usado pelo núcleo,
desempate timeout versus invalidação e eventos antigos após substituição foram
exercitados no host, não reproduzidos na VM.
Como há apenas um supervisor e somente ele pode encerrar ou reiniciar a rede,
não foi reproduzida em VM uma substituição enquanto ele próprio estava bloqueado
na observação; a VM observou a falha espontânea provocada por UD2. O fallback
de serviço novo com kernel ABI 2 antigo não foi combinado em VM. O supervisor
da imagem normal ainda não usa a observação pontual; ela foi exercitada pelo
serviço instrumentado. Notificações persistentes, espera multiplexada geral e o marco 0.3 completo continuam
pendentes. As evidências históricas foram restauradas após as regressões.
