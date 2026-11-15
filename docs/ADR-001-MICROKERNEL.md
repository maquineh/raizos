# ADR 001 — Microkernel próprio em Rust

**Estado:** aceito por decisão explícita do usuário durante a construção da base.
**Data de referência:** 9 de novembro de 2026, America/Boa_Vista.
**Escopo:** arquitetura do Raiz OS; não adota a implementação de outro SO.

## Contexto

O objetivo é um SO próprio, distribuído como produto integrado, com inicialização,
kernel, terminal, rede e crescimento posterior para persistência e aplicações.
O usuário definiu que o núcleo deve ser microkernel. O primeiro experimento de
boot, com console e rede no núcleo, serviu para provar a plataforma; sua divisão
de responsabilidades foi substituída antes desta entrega.

## Decisão

Manter em ring 0 os mecanismos que exigem autoridade sobre a máquina: transições
de privilégio, espaços de endereço, contextos de execução, preempção, tratamento
de exceções e interrupções, entrega de IPC e aplicação de permissões.

Manter terminal, protocolos de rede, drivers comuns, arquivos, interfaces
gráficas e políticas de administração em processos de usuário. Timer e
controlador de interrupções permanecem no núcleo para a arbitragem de CPU.

Usar Rust `no_std` nos componentes executados no SO, assembly embutido somente
nas fronteiras de CPU e contratos de ABI próprios, versionados e explícitos.

Entregar uma imagem integrada. A unidade de distribuição não determina a
unidade de isolamento: binários incorporados são carregados como processos
distintos, com páginas e permissões separadas.

## Concessões temporárias do bootstrap 0.1

- Dois processos e reservas estáticas, criados pelo núcleo a partir de ELFs
  incorporados; o supervisor/init em userspace será acrescentado depois.
- Terminal e seus drivers formam um serviço; driver e pilha de rede formam outro.
  Cada grupo já está fora do kernel, mas ainda não há isolamento entre driver
  e protocolo dentro do mesmo grupo.
- Permissões fixas por identidade de processo, sem capabilities dinâmicas.
- Descoberta e controle restrito de configuração PCI no bootstrap do núcleo.
  Gerência de dispositivos deve migrar para serviço com autoridade delimitada.
- Debug serial de emergência no núcleo; terminal normal em userspace.
- Filas IPC limitadas e chamadas não bloqueantes; reservas de memória fixas.
- Sem reinício e sem IOMMU. O driver autorizado a DMA continua confiável.

Essas concessões devem permanecer registradas. Acrescentar filesystem ou TCP
dentro do núcleo para acelerar entregas contrariaria esta decisão e exigiria
uma revisão explícita, não uma alteração silenciosa da arquitetura.

## Consequências positivas esperadas

Redução do conjunto privilegiado, diagnóstico por domínio de falha, possibilidade
de atualização e reinício de serviços, interfaces testáveis e concessão explícita
de acesso a objetos e hardware. Esses benefícios dependem da implementação dos
limites: separar arquivos ou crates não cria isolamento de execução.

## Custos aceitos

Desenhar protocolos IPC e ciclo de vida dos serviços passa a ser trabalho central.
Há transições de privilégio, agendamento e possíveis cópias nos caminhos de I/O.
Falhas parciais, filas cheias, cancelamento, timeouts, versões e revogação precisam
de semântica. O tempo inicial para chegar a um SO completo aumenta em relação a
colocar todos os serviços em um único domínio privilegiado.

Não se fixa uma perda percentual de desempenho. Mediremos latência de IPC,
mudança de contexto, tempo de wake-up, uso de CPU e throughput. Dados volumosos
usarão memória compartilhada autorizada e filas limitadas quando esse mecanismo
existir; mensagens de controle continuarão pequenas.

## Critérios verificáveis

1. Terminal e rede identificam CPL 3.
2. O terminal usa IPC para solicitar operações de rede.
3. O terminal não consegue usar a syscall PCI atribuída à rede.
4. Uma exceção no processo de rede o interrompe sem interromper terminal/núcleo.
5. Uma leitura de página supervisor pelo serviço é recusada pela MMU.
6. Um serviço em laço sem cooperação não monopoliza a CPU indefinidamente.
7. Logs e relatórios distinguem resultados observados de mecanismos previstos.

Reinício de serviço, confinamento de DMA, revogação e recuperação de estado são
critérios de versões seguintes, não propriedades presumidas da versão 0.1.

## Referências primárias de conceito e plataforma

- Rust bare metal: https://doc.rust-lang.org/rustc/platform-support/x86_64-unknown-none.html
- UEFI e ExitBootServices: https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html
- Proteção, paginação e interrupções x86: https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html
- Conceito de capabilities: https://docs.sel4.systems/Tutorials/capabilities.html
- IPC em microkernel: https://docs.sel4.systems/Tutorials/ipc.html
- Drivers userspace e DMA: https://trustworthy.systems/projects/drivers/

As referências de seL4 ilustram conceitos. O projeto não incorpora seL4, seus
bindings, seu microkernel, nem herda suas garantias ou provas formais.

