# Execução RZ-019 — limites de IPC, inscrições e esperas

Execução local sobre o commit `1ce0944`, com alterações não commitadas. As
[fontes](fontes.sha256) e os [artefatos](artefatos.sha256) identificam os
arquivos da execução final; os hashes identificam bytes, sem demonstrar
autenticidade ou compilação reproduzível. A ABI permanece na versão 2 e a
[ADR 009](../../docs/ADR-009-LIMITES-IPC.md) documenta as decisões e limites.

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
| `cargo test -p nucleo --lib` | Passou: 29 testes. [Log](testes-host-nucleo.log). |
| `cargo raiz compilar` e `cargo raiz verificar-imagem` | Passaram. [Build](build-normal.log), [verificação](verificar-imagem.log). |
| `cargo raiz compilar --teste-quota` e `cargo raiz testar-quota` | Passaram em imagem isolada. [Comando](comando-quota.log), [serial](../teste-quota-1794725073649578100/serial.log), [resultado](../teste-quota-1794725073649578100/resultado.txt). |
| `cargo raiz testar` | Passou. [Relatório](RELATORIO-SMOKE.md). |
| `cargo raiz testar-isolamento` | Passou em dois boots. [Relatório](RELATORIO-ISOLAMENTO.md). |
| `cargo raiz testar-recuperacao` | Passou em dois boots. [Relatório](RELATORIO-RECUPERACAO.md). |
| `cargo raiz compilar --teste-vida` e `cargo raiz testar-vida` | Passaram. [Serial](../teste-vida-1794725191226050200/serial.log), [resultado](../teste-vida-1794725191226050200/resultado.txt). |
| `cargo raiz compilar --teste-ciclo-espera` e `cargo raiz testar-ciclo-espera` | Passaram. [Serial](../teste-ciclo-espera-1794725214069687500/serial.log), [resultado](../teste-ciclo-espera-1794725214069687500/resultado.txt). |
| `cargo raiz compilar --teste-receber-bloqueante` e `cargo raiz testar-receber-bloqueante` | Passaram. [Serial](../teste-receber-bloqueante-1794725247332647900/serial.log), [resultado](../teste-receber-bloqueante-1794725247332647900/resultado.txt). |

## Cobertura e limites

No ensaio de quota, os serviços reais em CPL 3 enviaram 15 mensagens sequenciais
da rede à caixa do terminal. A 16ª da rede recebeu `FILA_CHEIA`; o supervisor
ocupou a posição restante e recebeu a mesma recusa ao tentar exceder a capacidade
total. O terminal verificou referência, payload e FIFO das 16 mensagens; após
drená-las, as duas origens enviaram novamente. A sincronização do cenário usa
esperas de 300 tiques por fase, podendo falhar sob atraso de escalonamento; o
resultado aprovado depende das verificações e dos marcadores distintos.

No host foram exercitadas as duas origens como principal, a volta do anel,
recusa sem inserir ou despertar, retirada e readmissão, purga pela referência
exata, limite compartilhado entre gerações do mesmo ID e ordem da FIFO. A
quota de 15 do supervisor, a readmissão após retirar apenas uma mensagem e a
purga de geração não foram demonstradas pelo novo ensaio de quota em VM. Os
ensaios de vida e ciclo repetidos na VM cobrem, nos seus próprios cenários,
inscrição, cancelamento, substituição e espera por geração. Os testes do modelo
no host cobrem ainda rejeição de espera duplicada e liberação dos slots.

Os limites atuais são estáticos: 16 posições por caixa; na caixa do terminal,
até 15 por origem lógica autorizada; uma espera e um slot de observação por
processo, com autoridade separada da identidade. Não há quota ou entrega de IRQ
implementada neste item. Não se demonstra suporte SMP, hardware físico, ausência
de toda perda de dispositivo ou conclusão do marco 0.3.

## Tentativa preservada

O [primeiro build do ensaio](build-quota-tentativa-1.log) falhou porque a
ferramenta repassou ao kernel uma feature de teste que só existe nos serviços.
A ferramenta foi corrigida; o [build final](build-quota.log) e o ensaio em QEMU
passaram. A revisão também identificou imports ausentes no binário do kernel
antes da compilação completa, que foram corrigidos. Os relatórios históricos de
outras execuções foram restaurados após as regressões; os relatórios desta
execução estão nesta pasta.
