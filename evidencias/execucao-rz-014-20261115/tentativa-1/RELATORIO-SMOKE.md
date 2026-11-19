# Teste de integração do Raiz OS

Resultado: **FALHOU**.

Duração: 12.902 segundos.

Ambiente solicitado: QEMU/TCG, pc, 1 CPU, 256 MiB, OVMF, e1000.

## Etapas concluídas

- Imagem verificada: GPT, FAT32 e BOOTX64.EFI íntegros.
- Boot completado: ExitBootServices, TERMINAL_PRONTO e serviço de rede em CPL=3 observados.

Datagramas UDP com resposta idêntica verificada: **0**. Bytes de payload verificados: **0**.

## Falha observada

```text
Tempo esgotado aguardando "PING OK 10.0.2.2" após 10 segundos
```

## Evidências e alcance

- `serial-smoke.log`: bytes recebidos do console serial.
- `qemu-smoke-stderr.log`: mensagens do emulador.

O comando encerra o emulador também em caso de falha. A instrução inválida do serviço de rede é provocada intencionalmente. O caso de isolamento só passa quando o terminal continua respondendo, o estado do serviço indica falha e um novo envio IPC à rede recebe o erro esperado.

Este resultado cobre a plataforma virtual declarada e os casos executados. O isolamento observado é parcial: não comprova IOMMU, contenção de DMA, reinício automático de serviços, preempção sob qualquer carga, suporte a hardware físico, TCP, persistência ou prontidão para produção.
