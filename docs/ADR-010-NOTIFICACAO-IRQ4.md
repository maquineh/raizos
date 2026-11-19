# ADR 010 — Notificação IRQ4 para o terminal

**Estado:** implementada e validada no recorte IRQ4 exclusiva de RZ-020; RZ-021 validou posteriormente seu uso pelo terminal normal.

## Contexto

RZ-020 requer notificação limitada, independente da FIFO IPC, para a UART
COM1 exclusiva do QEMU pc/i440fx. O PIC é baseado em borda; EOI sozinho não
elimina uma causa de dispositivo nem garante outra borda após o rearme.

## Decisão

Na CPU única, com IF=0, associar IRQ4 à referência atual do terminal. A linha
começa mascarada e só é armada por chamada autorizada. O handler mascara IRQ4,
registra pendência coalescida, envia EOI ao PIC mestre e desperta espera
elegível; IRQ0 permanece independente. A entrega usa sequência não reutilizada,
e somente confirmação da sequência entregue rearma a linha. Cancelamento da
instância mantém a linha mascarada. Exaustão da sequência retorna `RECURSOS`.
IPC e IRQ alternam em concorrência, e timeout não consome a outra fonte.

O terminal configura, drena e interpreta a UART. Após confirmar, verifica-a
novamente antes de dormir; trabalho já presente é processado localmente, sem
depender de nova borda. Assim, a fronteira do kernel não inclui registradores
da UART. A confirmação e a verificação local são partes necessárias do
protocolo; FIFO limitada ainda pode transbordar sob carga não atendida.

## Alternativas e consequências

Enfileirar IRQ como IPC perderia isolamento contra caixa cheia. Ler LSR no
kernel acrescentaria política do dispositivo ao mecanismo. Um protocolo
genérico para IRQ compartilhada exigiria identificar fontes, roteamento e
confinamento ainda não implementados. Este recorte não os oferece, nem promete
perda zero sob fluxo ilimitado, hardware físico, SMP ou tempo real. O handler
também aceita IRQ4 durante idle CPL 0. O [ensaio opt-in em QEMU](../evidencias/teste-irq-1794725963506321900/resultado.txt) observou despertar, autoridade, confirmação e rearme. Em 50 tiques com trabalho pendente e sem confirmar, duas amostras indicaram máscara ativa e o contador permaneceu estável enquanto o timer avançou; isso não demonstra ausência absoluta de tempestade. A coalescência foi testada no host. Construção normal, verificação da imagem e cinco boots de regressão passaram. Na validação RZ-020, o terminal normal ainda usava polling; a [ADR 011](ADR-011-TERMINAL-UART.md) registra a integração posterior de IRQ4 no terminal. Hardware físico, SMP e IRQ compartilhada não foram demonstrados.
