# ADR 002 — Supervisor, referências de instância e recuperação

**Estado:** decisão adotada para a etapa 0.2.  
**Data:** 9 de novembro de 2026, America/Boa_Vista (UTC−04).  
**Escopo:** recuperação do serviço de rede na plataforma virtual de referência.

## Contexto

A versão 0.1 contém uma falha de CPU de um serviço, mas exige reboot para
recuperá-lo. A evolução autorizada pelo usuário deve preservar o microkernel e
transformar a recuperação em comportamento observável, com fronteiras de
autoridade e invalidação do estado da instância antiga.

Reutilizar apenas o identificador lógico `2` para uma nova rede permitiria que
pedidos antigos alcançassem a nova instância sem que o cliente percebesse. Além
disso, os buffers da rede podem permanecer acessíveis ao dispositivo por DMA.
Ainda não existe IOMMU nem prova de drenagem de todas as transações pendentes.

## Decisão

### Política em um terceiro processo de usuário

Criar `servico-supervisor`, identidade lógica 3, executando em CPL 3, sem portas
de I/O ou janelas MMIO. Ele administra exclusivamente a recuperação da rede.
Terminal e rede permanecem nas identidades 1 e 2.

O supervisor decide se uma recuperação foi solicitada pelo usuário ou por sua
política automática. O kernel verifica autorização, prepara o novo espaço,
retira a instância anterior do agendamento, invalida mensagens e publica o novo
contexto. O kernel não incorpora a política de retentativas.

O modo inicial é manual. `supervisao auto` habilita observação de falha ou término;
`supervisao manual` desabilita a recuperação automática. `parar-rede` também muda
para manual antes de encerrar, para que uma parada intencional seja estável.
Um laço infinito não é diagnosticado automaticamente nesta versão; o usuário
pode pedir `reiniciar-rede` enquanto a preempção preserva terminal e supervisor.

### Referência com geração

Toda operação vinculada a uma instância recebe uma referência de 64 bits:

```text
referencia = (geracao << 32) | identificador_logico
```

A geração começa em 1. `RESOLVER(id)` encontra a referência atual, inclusive
quando a instância falhou ou terminou. `ENVIAR`, `ESTADO_SERVICO`, `REINICIAR` e
`ENCERRAR` recebem a referência completa. Uma geração antiga recebe
`REFERENCIA_ANTIGA`.

O remetente das mensagens é preenchido pelo kernel com sua referência real. A
referência identifica uma instância; não concede autoridade por si só. As
permissões continuam sendo uma política estática separada, verificada no kernel.

O grafo IPC permite terminal↔rede e terminal↔supervisor. A rede não recebe
permissão de reiniciar serviços. O terminal solicita a operação ao supervisor e
não pode invocar diretamente os mecanismos privilegiados de ciclo de vida.

### Preparação e publicação

O reinício valida a identidade do chamador, a referência alvo e o orçamento antes
de alterar uma instância em execução. A imagem nova é preparada em uma arena e
um conjunto de tabelas inativos. Se a preparação falhar, a instância anterior
permanece publicada.

Após a preparação bem-sucedida, com interrupções desabilitadas na CPU única:

1. Desabilitar bus mastering da NIC da instância anterior.
2. Retirar a instância anterior da execução.
3. Esvaziar sua caixa de mensagens e remover das demais caixas as mensagens que
   tenham sua referência como origem.
4. Instalar contexto, páginas e informações da nova instância.
5. Publicar a geração nova e retorná-la ao supervisor.

O cliente recebe erro para a referência antiga. A nova instância começa com
estatísticas e estado de protocolos vazios. A versão 0.2 não restaura sessões,
pedidos de ping pendentes ou estado de aplicações.

### Memória DMA em quarentena e orçamento

Reservar quatro arenas de 2 MiB para a rede, além das arenas de terminal e
supervisor. Cada encarnação usa uma arena e tabelas próprias. As arenas de rede
anteriores permanecem reservadas até o reboot e não são reaproveitadas por outro
serviço ou por outra encarnação.

Há **quatro encarnações por boot: a original e três reinícios**. Quando o
orçamento termina, o kernel retorna `RECURSOS` antes de derrubar a instância
atual. O supervisor deve informar o esgotamento e interromper retentativas
automáticas infrutíferas.

Essa solução é uma concessão temporária e limitada de laboratório. Não cria
isolamento contra DMA malicioso: o driver continua confiável. A devolução e o
reuso seguro dessas arenas dependem de trabalho posterior sobre IOMMU e ciclo de
vida dos dispositivos.

### Reinicialização da placa no serviço de rede

O kernel delega a janela MMIO com bus mastering inicialmente desligado. O driver
desliga RX/TX, mascara interrupções, solicita reset com espera limitada e
reprograma descritores, endereços e índices para sua arena nova. Somente então
habilita DMA e o tráfego.

O manual Intel informa que reset do controlador não equivale a apagar todo o
estado de configuração; a implementação deve programar explicitamente os
registradores necessários. [1] O alvo verificado nesta etapa continua sendo a
e1000 emulada no QEMU.

## Consequências

- A ABI de serviços passa à versão 2. Todos os serviços e o núcleo são
  reconstruídos juntos; binários ABI 1 não são compatíveis.
- A distribuição continua sendo uma imagem única, agora com três processos.
- Reinício não aumenta indefinidamente a reserva de RAM: o limite é estático e
  produz erro explícito.
- Políticas e mecanismos ficam em componentes diferentes, preservando a decisão
  de microkernel.
- A política automática monitora estados por consulta; IPC bloqueante,
  notificações de morte e watchdog de serviço travado ficam para a etapa seguinte.
- Criação geral de processos, execução de aplicações e recuperação do próprio
  supervisor ainda não são oferecidas.

## Critérios de aceitação

1. Supervisor executa em CPL 3 e o terminal não consegue invocar diretamente
   reinício ou encerramento da rede.
2. Após falha da rede, uma solicitação ao supervisor cria a geração seguinte
   sem novo boot; ping e UDP voltam a funcionar.
3. Envios e consultas à referência guardada antes do reinício são recusados.
4. Filas circulares removem a origem revogada, preservando a ordem e o conteúdo
   das mensagens de outras instâncias.
5. Com modo automático, uma falha de memória provocada na rede resulta em
   recuperação e retomada do tráfego.
6. Parada intencional fica estável; reinício posterior restaura o serviço.
7. Esgotamento das quatro encarnações retorna erro e preserva uma rede ainda viva.
8. Os cenários de isolamento e preempção da versão anterior continuam passando.

Resultados observados pertencem aos relatórios em `evidencias/`. Este ADR define
a decisão e seus critérios; não substitui a execução dos testes.

## Referência primária

1. Intel. *PCI/PCI-X Family of Gigabit Ethernet Controllers Software Developer's
   Manual*, revisão 4.0, seções 13.4.1 e 14.7.
   https://www.intel.com/content/dam/doc/manual/pci-pci-x-family-gbe-controllers-software-dev-manual.pdf
