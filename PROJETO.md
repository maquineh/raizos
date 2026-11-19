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

RZ-014 foi concluído no escopo de estados, despertar por IPC e idle: há estados de vida e agendamento, idle e despertar por IPC admitido. Um ensaio opt-in em QEMU usa terminal e supervisor em CPL 3 para exercitar esse evento; o [ensaio anterior de idle](evidencias/execucao-rz-014-20261115/final/RELATORIO-TESTE-IDLE.md) usava despertar artificial. A [ADR 004](docs/ADR-004-ESPERA-IPC-ABI-2.md) adiciona espera de disponibilidade à ABI 2: o supervisor manual usa IPC/vida com fallback para recepção bloqueante e essa espera. Na execução de RZ-018, o modo automático ainda consultava estado e prazos por polling; no RZ-022, essa consulta contínua foi substituída por espera de vida/IPC e prazo de retentativa. Kernels antigos retornam `CHAMADA`, com fallback cooperativo; a combinação de kernel antigo e serviço novo não foi ensaiada em VM. As regressões históricas na imagem normal reconstruída passaram; ABI 2 e `RECEBER` não bloqueante permanecem vigentes. RZ-015 está concluído para recepção IPC bloqueante da própria caixa, conforme a [ADR 005](docs/ADR-005-RECEPCAO-BLOQUEANTE.md) e o [resumo de validação](evidencias/execucao-rz-015-20261115/RESUMO.md); RZ-016 está concluído para prazos de recepção IPC da própria caixa, conforme a [ADR 006](docs/ADR-006-PRAZOS-IPC.md) e o [resumo da execução](evidencias/execucao-rz-016-20261115/RESUMO.md). RZ-017 está concluído no recorte de cancelamento por ciclo de vida e observação pontual autorizada, conforme a [ADR 007](docs/ADR-007-ESPERA-E-CICLO-DE-VIDA.md) e o [resumo da execução](evidencias/execucao-rz-017-20261115/RESUMO.md). RZ-018 está concluído no recorte supervisor→rede: a [ADR 008](docs/ADR-008-NOTIFICACOES-VIDA.md) introduz inscrição persistente supervisor→rede e espera de IPC/vida na ABI 2. O [ensaio de vida](evidencias/teste-vida-1794724542137348500/resultado.txt) e as regressões em QEMU passaram. Os cinco ensaios específicos anteriores reconstruídos passaram, conforme o [resumo da execução](evidencias/execucao-rz-018-20261115/RESUMO.md); as regressões gerais também passaram novamente na imagem normal final. A espera multiplexada completa e o marco 0.3 permanecem pendentes.

RZ-019 está concluído no recorte implementado: a [ADR 009](docs/ADR-009-LIMITES-IPC.md) descreve caixas de 16 mensagens e limite de 15 por origem lógica na caixa do terminal; inscrição e espera usam slots estruturais por processo. Passaram 29 testes do núcleo no host; as regressões normais em QEMU passaram. RZ-020 está concluído no recorte de IRQ4 exclusiva. O [ensaio de quota em QEMU](evidencias/teste-quota-1794725073649578100/resultado.txt) passou para 15 envios da rede, recusa do 16º, reserva ao supervisor, FIFO e readmissão após drenagem. A ordem inversa e liberação após uma retirada ou purga foram testadas no host, não nesse ensaio de VM. O [resumo da execução](evidencias/execucao-rz-019-20261115/RESUMO.md) reúne construção normal, verificação de imagem e os cinco boots de regressão.


RZ-020 está concluído no recorte de IRQ4 exclusiva: a [ADR 010](docs/ADR-010-NOTIFICACAO-IRQ4.md) acrescenta à ABI 2 uma associação exclusiva de IRQ4 ao terminal, notificação coalescida separada da fila IPC e confirmação por sequência. No [ensaio opt-in em QEMU](evidencias/teste-irq-1794725963506321900/resultado.txt), IRQ4 acordou o terminal a partir de idle CPL 0; foram observadas negação a outro serviço, recusas de sequência incorreta e duplicada, rechecagem e rearme. Com B pendente e sem confirmação por 50 tiques, a máscara foi observada em duas amostras, o contador de entradas permaneceu igual e o timer avançou; C chegou após o rearme. Coalescência foi testada no host, não demonstrada nesse ensaio QEMU. Isso não demonstra ausência absoluta de tempestades nem suporte a hardware físico. A construção normal, verificação de imagem e três comandos de regressão em QEMU (cinco boots) passaram. Naquela execução o terminal normal ainda usava polling. SMP e IRQ compartilhada não foram demonstrados. Os ensaios reconstruídos de quota, vida e prazo-idle também passaram; o [resumo da execução RZ-020](evidencias/execucao-rz-020-20261115/RESUMO.md) reúne logs, hashes e limites.

A implementação deverá demonstrar que um evento não se perde entre a verificação
da fila e o bloqueio, que cada espera termina uma única vez e que um serviço sem
trabalho deixa de disputar CPU. O piloto de interrupção proposto é a UART COM1,
IRQ4 exclusiva na máquina de referência. Rede e PS/2 podem aguardar prazos de
polling; o encaminhamento de interrupções PCI da NIC exige tratamento próprio.

Os cenários completos de aceitação da 0.3 ainda não foram executados. A nova versão só será
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


RZ-021 está concluído no recorte validado. O terminal normal agora usa IRQ4/IPC e prazo para PS/2, conforme a [ADR 011](docs/ADR-011-TERMINAL-UART.md). O [ensaio instrumentado](evidencias/teste-uart-1794726895277854900/resultado.txt) passou, inclusive na janela de rearme controlada; o ensaio normal, construção, verificação de imagem e cinco boots normais também passaram. Os ensaios adicionais de IRQ, quota, vida e prazo-idle passaram; no host passaram 31 testes do núcleo e 23 dos pacotes. O PS/2 emulado por HMP executou o comando sobre e recebeu resposta distinta. As rajadas ensaiadas foram de quatro bytes a cada 50 ms. PS/2 físico e fallback com kernel antigo não foram validados em VM; não há garantia de perda zero geral.

RZ-022 está concluído no escopo validado. A [ADR 012](docs/ADR-012-ESPERA-SERVICOS-RZ022.md) e o [resumo da execução final](evidencias/execucao-rz-022-20261115/RESUMO.md) registram os dois boots integrados, o ciclo de espera aprovado duas vezes, a imagem normal, as regressões e os ensaios complementares. A e1000 conserva polling RX com prazo; hardware físico, PS/2 físico e fallback em kernel ABI 2 antigo não foram validados. RZ-023 está concluído no escopo do aceite original; seus 12 comandos opt-in, testes host e cinco boots normais estão no [resumo](evidencias/execucao-rz-023-20261115/RESUMO.md) e na [matriz T03](evidencias/execucao-rz-023-20261115/MATRIZ.md). RZ-024 foi concluído no recorte controlado de IRQ4, prazo e pressão finita, conforme o [resumo](evidencias/execucao-rz-024-20261115/RESUMO.md). A matriz T03 mantém lacunas por cenário; nem todos os T03 estão demonstrados e o marco 0.3 continua incompleto. ABI 2 e versão 0.2.0 permanecem vigentes.
