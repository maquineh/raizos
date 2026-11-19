# Teste de integração do Raiz OS

Resultado: **APROVADO**.

Duração: 16.621 segundos.

Ambiente solicitado: QEMU/TCG, pc, 1 CPU, 256 MiB, OVMF, e1000.

## Etapas concluídas

- Imagem verificada: GPT, FAT32 e BOOTX64.EFI íntegros.
- Boot completado: ExitBootServices, TERMINAL_PRONTO e serviço de rede em CPL=3 observados.
- Comando sobre informou CPL=3 do terminal; ICMP echo do gateway 10.0.2.2 recebido.
- UDP eco verificado com payloads de 0, 1, 17 e 1472 bytes.
- 80 datagramas UDP adicionais verificados sequencialmente, cobrindo a volta dos anéis RX/TX.
- Comando rede confirmou RX/TX, ping_ok=1 e udp_eco=84.
- Acesso sem capacidade foi negado e confirmado pelo comando teste-cap.
- Instrução inválida provocada no serviço de rede gerou falha id=2, vetor=6.
- Após a morte da rede, terminal respondeu ao eco, processos informou rede=2 e o novo ping recebeu erro de serviço indisponível por IPC.

Datagramas UDP com resposta idêntica verificada: **84**. Bytes de payload verificados: **6610**.

## Evidências e alcance

- `serial-smoke.log`: bytes recebidos do console serial.
- `qemu-smoke-stderr.log`: mensagens do emulador.

O comando encerra o emulador também em caso de falha. A instrução inválida do serviço de rede é provocada intencionalmente. O caso de isolamento só passa quando o terminal continua respondendo, o estado do serviço indica falha e um novo envio IPC à rede recebe o erro esperado.

Este resultado cobre a plataforma virtual declarada e os casos executados. O isolamento observado é parcial: não comprova IOMMU, contenção de DMA, reinício automático de serviços, preempção sob qualquer carga, suporte a hardware físico, TCP, persistência ou prontidão para produção.
