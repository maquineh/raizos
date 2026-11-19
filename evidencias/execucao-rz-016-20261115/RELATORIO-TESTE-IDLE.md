# Teste controlado de idle

Resultado: **APROVADO**. Duração: 3.362 segundos.

Ambiente solicitado: QEMU/TCG, pc, 1 CPU, 256 MiB, OVMF e e1000. Imagem: `saida/teste-idle/raiz-os.img`.

## Marcadores observados

Os quatro marcadores de teste são exigidos na ordem indicada. TERMINAL_PRONTO deve ocorrer após SERVICOS_PRONTOS, podendo ocorrer antes ou depois de RETORNO_CPL3.

- `[teste-idle] TODOS_BLOQUEADOS`
- `[teste-idle] IRQ0_CPL0`
- `[teste-idle] SERVICOS_PRONTOS`
- `[teste-idle] RETORNO_CPL3`
- `[teste] TERMINAL_PRONTO`

Logs: `serial-teste-idle.log` e `qemu-teste-idle-stderr.log`. O cenário instrumentado é opt-in e não demonstra espera por eventos dos serviços nem desempenho geral.
