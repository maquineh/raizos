# Projeto Raiz OS

**Ponto de entrada para continuar o desenvolvimento.**

| Campo | Definição |
|---|---|
| Produto | Raiz OS, nome provisório |
| Responsável pelo projeto | Raphael Maquiné Marinho |
| Objetivo | Sistema operacional próprio em Rust, com microkernel e entrega integrada |
| Base executável | 0.2.0, ABI de serviços 2 |
| Situação | SO experimental mínimo de propósito restrito |
| Próximo marco proposto | 0.3: espera bloqueante, prazos e notificações |
| Plataforma comprovada | x86-64, UEFI/OVMF, QEMU pc/i440fx, 1 CPU, 256 MiB, e1000 |

Esta organização do projeto acrescenta requisitos, plano de trabalho e regras de
continuidade à versão 0.2. Não implementa a versão 0.3 nem muda a imagem do SO.

## 1. Compromissos do produto

Construiremos carregador, microkernel, serviços, drivers, protocolos e ambiente de
aplicações próprios. O código do sistema é Rust, incluindo as regiões `unsafe`
e o assembly embutido necessários ao hardware. A implementação não usa outro
kernel ou distribuição como base. Compilador, firmware e emulador são ferramentas
ou partes da plataforma, documentadas separadamente.

O microkernel administra mecanismos de CPU, memória, proteção, interrupções e
comunicação. Terminal, política de recuperação, protocolos e drivers de serviço
executam fora do núcleo. A evolução deve preservar essa divisão.

A distribuição é um produto integrado. Hoje, uma imagem de disco incorpora o
carregador, o núcleo e três serviços com espaços de endereço separados. Futuras
aplicações, persistência e atualização devem manter uma instalação coerente e
uma interface compreensível para o usuário.

A avaliação usa os conceitos de abstração e administração de recursos discutidos
por Tanenbaum. Os critérios operacionais são definidos pelo projeto e ligados a
evidências; não representam certificação do autor. Consulte a
[matriz de avaliação](docs/CRITERIOS-TANENBAUM.md).

## 2. O que já existe

| Área | Entrega da base 0.2 |
|---|---|
| Inicialização | Carregador UEFI próprio, kernel ELF incorporado, saída dos serviços de boot |
| Microkernel | Ring 0/ring 3, tabelas de páginas por processo, preempção, syscalls e IPC limitado |
| Terminal | Console serial, framebuffer, teclado PS/2 e comandos próprios |
| Rede | Driver e1000, Ethernet, ARP, IPv4, ICMP e eco UDP |
| Recuperação | Supervisor separado, identidade com geração e até três reinícios da rede por boot |
| Entrega | Imagem GPT/FAT32 de 128 MiB e ferramentas de construção em Rust |

A validação registrada da 0.2 contém **26 testes no host e cinco boots em QEMU**.
Esses resultados são históricos da implementação entregue. A formalização do
projeto verifica documentação, rastreabilidade e integridade do pacote; não é
uma nova execução desses testes. Veja o
[resumo de validação](evidencias/RESUMO-VALIDACAO.md) e o
[registro desta organização](evidencias/REVISAO-PROJETO.md).

Ainda faltam execução geral de aplicações, filesystem e persistência, memória
dinâmica com política definida, TCP/DHCP/DNS, confinamento DMA por IOMMU e suporte
a uma matriz de hardware real. O driver DMA atual precisa ser confiável. Esses
limites integram o [modelo de ameaças](docs/projeto/AMEACAS-E-LIMITES.md).

## 3. Documentos de trabalho

| Documento | Uso |
|---|---|
| [PRD](docs/projeto/PRD.md) | Escopo, requisitos e condições de promoção do produto |
| [Roadmap](docs/projeto/ROADMAP.md) | Ordem proposta dos marcos e suas dependências |
| [Backlog CSV](docs/projeto/BACKLOG.csv) | Tarefas identificadas, status, dependências, aceitação e evidências |
| [Especificação 0.3](docs/projeto/ESPECIFICACAO-0.3.md) | Contrato proposto para espera, deadlines, eventos e piloto de IRQ |
| [Ameaças e limites](docs/projeto/AMEACAS-E-LIMITES.md) | Fronteiras de confiança e garantias ainda não demonstradas |
| [Validação e entrega](docs/projeto/VALIDACAO-E-ENTREGA.md) | Comandos, evidências e critérios para aceitar uma alteração |
| [Como contribuir](CONTRIBUINDO.md) | Preparação do ambiente e fluxo de desenvolvimento |
| [Instruções para agentes](AGENTS.md) | Regras técnicas para continuar o código com assistência |
| [Continuidade](docs/projeto/CONTINUIDADE.md) | Contexto e tarefa inicial para a próxima sessão |

O [README](README.md) explica a execução atual. A
[arquitetura](docs/ARQUITETURA.md), as decisões
[ADR-001](docs/ADR-001-MICROKERNEL.md) e
[ADR-002](docs/ADR-002-CICLO-DE-VIDA.md), e a
[ABI 0.2](docs/ABI-0.2.md) descrevem o que já está implementado.

## 4. Próximo trabalho concreto

O próximo incremento começa pelo contrato da espera bloqueante da versão 0.3.
O documento disponível é uma proposta técnica; a ABI vigente continua sendo 2.
O primeiro item do marco deve fechar estados, resultados, tratamento de prazos,
cancelamento por morte/reinício e regras de autoridade antes de expor novas
operações aos serviços.

A implementação deverá demonstrar que um evento não se perde entre a verificação
da fila e o bloqueio, que cada espera termina uma única vez e que um serviço sem
trabalho deixa de disputar CPU. O piloto de interrupção proposto é a UART COM1,
IRQ4 exclusiva na máquina de referência. Rede e PS/2 podem aguardar prazos de
polling; o encaminhamento de interrupções PCI da NIC exige tratamento próprio.

Os cenários de aceitação da 0.3 ainda não foram executados. A nova versão só será
declarada implementada depois de código, integração dos serviços e evidências
correspondentes, mantendo as verificações da 0.2 aplicáveis.

## 5. Como usar este projeto

1. Extraia o pacote e abra a pasta `raiz-os` no editor.
2. Leia este arquivo e o [guia de contribuição](CONTRIBUINDO.md).
3. Instale as ferramentas externas indicadas no README e confira a imagem atual.
4. Escolha um item do backlog e trabalhe em uma alteração com aceitação explícita.
5. Registre código, decisões, resultado observado e limites na mesma entrega.

O pacote inclui um repositório Git local, na branch `main`, com a marca
`projeto-base-0.2.0`. Ele começa na importação desta base; não reconstrói um
histórico de commits das versões anteriores. O histórico funcional permanece no
[CHANGELOG](CHANGELOG.md) e nas evidências preservadas.

Repositório remoto, licença de distribuição e prioridades após o próximo marco
permanecem escolhas do responsável pelo projeto.

Para retomar o desenvolvimento em outra sessão ou ferramenta, entregue a pasta
completa e use o [roteiro de continuidade](docs/projeto/CONTINUIDADE.md).
