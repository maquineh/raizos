# Execução RZ-014 — infraestrutura de ociosidade

Esta execução local parte do commit `1ce0944` com alterações não commitadas;
os hashes dos arquivos de fonte e configuração ao final estão em
[`final/fontes.sha256`](final/fontes.sha256). Após as regressões, apenas o texto
do relatório de idle no harness foi esclarecido; o cenário de idle foi repetido.
Não houve mudança no código do convidado após os builds registrados. A base executável continua em
0.2.0, ABI 2. O item RZ-014 está em andamento: bloqueio e despertar por eventos
reais ainda não foram integrados.

## Ambiente e artefatos

- Host Windows; rustc 1.99.1 (58cf8f9c3 2026-10-13), Cargo 1.99.1
  (5f94df478 2026-08-27).
- QEMU 10.2.50 (v10.2.0-14086-g840320663e), TCG, máquina pc/i440fx,
  uma CPU, 256 MiB, e1000 e OVMF. CODE SHA-256:
  `33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6db2a`;
  VARS SHA-256:
  `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e`.
- Imagem normal SHA-256:
  `bf51d2d2ac09ae229b1a93e82f6f07553b4ebbc58bec8928e2a4a2fdbea2685b`;
  EFI normal:
  `d6a92aa620f577db912549f214e7fc7e891c9f6cbf59398df21722f143cf2fb9`.
- Imagem com `--teste-idle` SHA-256:
  `62d21d347f84e9e69335f3954a6fcb28a686bf0c5b3309cf3d758af78c702c9f`;
  EFI de teste:
  `5fb3d9971272181e0a6996358584219b960ec4bdd536baf881b30f6fdd02f9fd`.

As imagens são artefatos locais em `saida/`; seus hashes identificam os bytes
testados, não demonstram compilação reproduzível nem autenticidade. Os logs de
[construção normal](final/build-normal.txt) e
[construção instrumentada](final/build-idle.txt) acompanham este registro.

## Verificações

| Comando | Resultado observado |
| --- | --- |
| `cargo fmt --all --check` | Passou. |
| `cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema` | Passou: 17 testes; contrato-boot tinha zero testes. [Log](final/testes-host-pacotes.log). |
| `cargo test -p nucleo --lib` | Passou: 11 testes. [Log](final/testes-host-nucleo.log). |
| `cargo raiz compilar` e `cargo raiz verificar-imagem` | Passaram; estrutura GPT/FAT32 e bytes EFI verificados. |
| `cargo raiz compilar --teste-idle` e `cargo raiz testar-idle` | Passaram em imagem separada. [Relatório](final/RELATORIO-TESTE-IDLE.md). |
| `cargo raiz testar` | Passou. [Relatório](final/RELATORIO-SMOKE.md). |
| `cargo raiz testar-isolamento` | Passou nos dois boots. [Relatório](final/RELATORIO-ISOLAMENTO.md). |
| `cargo raiz testar-recuperacao` | Passou nos dois boots. [Relatório](final/RELATORIO-RECUPERACAO.md). |

O cenário instrumentado bloqueou os três serviços antes de sua primeira
execução, recebeu IRQ0 em CPL 0 e os tornou prontos artificialmente na terceira
interrupção; observou retorno a CPL 3 e terminal pronto. Isso exercita o
contexto ocioso, não prova espera por IPC, prazos, cancelamento ou notificações.

## Tentativas e preservação

Os relatórios em [`tentativa-1`](tentativa-1/) registram falhas reais: os
comandos seriais foram truncados nas primeiras execuções de smoke e isolamento,
e o cenário de recuperação com rede em loop falhou usando intervalo de 5 ms
por byte. Os logs mostram a entrada incompleta; isso sugere perda na serial,
sem estabelecer a causa de hardware de forma definitiva. Smoke e isolamento
passaram com 5 ms em [`tentativa-2`](tentativa-2/). O harness final envia bytes
com 50 ms de intervalo e os três comandos de regressão passaram. Esse intervalo
é uma mitigação experimental, não garantia geral contra perda de entrada.

As evidências históricas 0.2 foram copiadas antes dos testes e restauradas nos
seus caminhos originais. Os resultados desta execução estão neste diretório;
eles não substituem nem modificam o alcance das evidências históricas.
