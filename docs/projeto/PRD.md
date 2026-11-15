# PRD — Raiz OS

**Estado:** projeto experimental em desenvolvimento.  
**Base implementada:** 0.2.0, ABI de serviços 2.  
**Próximo marco planejado:** 0.3, serviços orientados a eventos.  
**Planejamento relacionado:** [Roadmap](ROADMAP.md).

## 1. Visão e público inicial

Construir um sistema operacional próprio em Rust, com microkernel e serviços
isolados, entregue como um produto integrado. O desenvolvimento deve avançar por
capacidades demonstráveis: executar programas, administrar recursos, comunicar
serviços, conter falhas e recuperar componentes dentro de um modelo de confiança
declarado.

Os primeiros usuários são desenvolvedores de sistemas e pesquisadores que querem
estudar, modificar e verificar um SO em uma máquina virtual controlada. A entrega
atual é uma base executável de pesquisa. O ambiente de aplicações e o produto de
uso geral permanecem objetivos de evolução.

| Usuário inicial | Necessidade | Resultado esperado |
|---|---|---|
| Desenvolvedor de kernel ou serviços | Alterar um componente e observar seu efeito | Reconstruir a imagem, executar cenários documentados e relacionar falhas ao componente responsável |
| Pesquisador de sistemas | Avaliar isolamento, comunicação e recuperação | Reproduzir casos com logs, configuração da VM e limites da evidência disponíveis |
| Colaborador do projeto | Implementar uma entrega sem redesenhar todo o SO | Encontrar requisito, contrato, dependências, critério de aceitação e decisão arquitetural correspondente |

O sucesso inicial é permitir esse trabalho com continuidade e resultados
verificáveis. Quantidade de funcionalidades, aparência do terminal e desempenho
não medido não substituem os critérios de aceitação.

## 2. Ponto de partida e alcance da evidência

A versão 0.2 tem carregador UEFI próprio, microkernel, terminal, rede e supervisor
em processos separados. O registro de validação contém 26 testes no host e cinco
boots de integração, incluindo preempção, proteção de memória, rede e recuperação.
Esses resultados pertencem à execução registrada da 0.2, na plataforma x86-64,
QEMU/TCG `pc`/i440fx, uma CPU, 256 MiB, OVMF e e1000 emulada.
Fonte: [resumo da validação](../../evidencias/RESUMO-VALIDACAO.md).

A recuperação limita-se à rede: quatro encarnações por boot, com arenas físicas
novas e antigas em quarentena. O driver DMA continua confiável; não há IOMMU.
Não há aplicações dinâmicas, persistência após o boot ou suporte validado a
hardware físico. A purga de filas ocupadas foi verificada no host; nos cenários
de recuperação em VM, as filas estavam vazias durante a troca.
Fontes: [ADR 002](../ADR-002-CICLO-DE-VIDA.md) e
[relatório de recuperação](../../evidencias/RELATORIO-RECUPERACAO.md).

A classificação adotada é **SO experimental mínimo de propósito restrito**.
As perspectivas de Tanenbaum sobre abstrações e administração de recursos
orientam a avaliação; os requisitos abaixo são decisões de engenharia deste
projeto. Ring 3, preempção, GUI, POSIX e filesystem não são apresentados como
condições universais impostas pelo autor.
Fonte: [avaliação pelos conceitos de Tanenbaum](../CRITERIOS-TANENBAUM.md).

## 3. Requisitos e critérios verificáveis

Os IDs são estáveis e devem ser citados nas tarefas e nos relatórios. “Permanente”
indica uma restrição do produto; “regressão 0.2”, um comportamento a preservar;
“0.3” e “futuro”, entregas planejadas, ainda não comprovadas pela versão atual.
Uma mudança de escopo deve atualizar a decisão, o contrato e a evidência afetados.

### Identidade do produto

| ID | Requisito permanente | Verificação |
|---|---|---|
| RQ-001 | Manter implementação própria do carregador, kernel e serviços essenciais, sem usar Linux, BSD, Redox, seL4 ou outro SO como base de execução. | Revisão das fontes e dependências; boot do firmware até serviços próprios sem SO hospedeiro dentro do convidado. Padrões de interoperabilidade e ferramentas externas permanecem explicitados. |
| RQ-002 | Implementar os componentes do SO em Rust. Assembly embutido, `unsafe`, ligação e interfaces de hardware devem ter finalidade e invariantes documentados. | Inventário dos componentes e dependências; revisão das fronteiras privilegiadas. Firmware, compilador, microcódigo e emulador são dependências externas, não uma alegação de “100% safe Rust”. |
| RQ-003 | Preservar o microkernel: mecanismos de CPU, memória, IPC e autoridade no núcleo; protocolos, interfaces, drivers de serviço e política de recuperação em processos de usuário. | Revisão de responsabilidades e teste de falha de serviço com continuidade dos demais. As concessões estáticas do bootstrap ficam registradas em ADR. |
| RQ-004 | Entregar uma distribuição integrada, com versão, contratos e imagem de boot coerentes. Serviços internos podem ser binários distintos. | Construir e verificar a imagem a partir da árvore identificada; inicializar seus componentes compatíveis. Uma imagem única não exige um único processo ou domínio de privilégio. |

### Comportamentos da base 0.2

| ID | Requisito de regressão | Verificação |
|---|---|---|
| RQ-005 | Preservar preempção e separação dos acessos de CPU entre serviços e núcleo. | Rede em laço sem ceder não impede resposta do terminal; leitura da página supervisor ensaiada causa falha apenas do serviço infrator. Novas permissões exigem casos correspondentes. |
| RQ-006 | Mediar IPC por identidade real, referências de instância, permissões e filas limitadas. | Recusar operação sem direito, referência antiga e fila cheia; conservar FIFO e remover mensagens revogadas preservando as demais. Registrar se cada caso foi exercitado no host ou na VM. |
| RQ-007 | Preservar recuperação autorizada da rede, sem reutilizar suas arenas antigas no contrato 0.2. | Reinício manual/automático e parada/retomada restabelecem ping e UDP; terminal não gerencia diretamente; quinta encarnação é recusada sem derrubar a quarta ainda viva. |
| RQ-008 | Manter construção documentada e evidência rastreável por versão. | Registrar fontes/revisão, toolchain, configuração, comandos, resultado, logs e hashes dos artefatos. Distinguir revisão, teste no host e execução do convidado; não exigir identidade binária entre ambientes sem demonstrá-la. |

### Próximo marco: 0.3

| ID | Requisito planejado | Critério de aceitação |
|---|---|---|
| RQ-009 | Permitir espera bloqueante por comunicação e eventos, separando estado de vida do serviço de seu estado no escalonador. | Processo bloqueado sai do conjunto executável sem ser tratado como morto; mensagem disponível evita bloqueio; chegada antes, durante ou depois da transição de espera não perde o despertar. Com todos bloqueados, o núcleo mantém um caminho de espera ociosa e retorno por evento. |
| RQ-010 | Oferecer timeout e cancelamento de espera com semântica explícita de término, falha e mudança de geração do destino. | Exercitar prazo expirado, chegada no limite, cancelamento e reinício enquanto há cliente esperando. Cada espera conclui uma vez, com resultado documentado, sem alcançar outra encarnação por referência antiga. |
| RQ-011 | Entregar notificações de IRQ autorizadas e manter limites de recursos sob carga. O piloto planejado é a UART COM1, com IRQ4 exclusiva. | IRQ real acorda o terminal; serviço sem direito é recusado. Notificações de IRQ/ciclo de vida permanecem observáveis com a fila IPC cheia; repetição de eventos e cliente parado respeitam limites, sem crescimento ilimitado ou perda da preempção. |

### Capacidades futuras

| ID | Requisito de evolução | Critério de aceitação |
|---|---|---|
| RQ-012 | Administrar memória e objetos de autoridade dinamicamente, com limites e revogação. | Alocar e devolver recursos; recusar acesso sem direitos e referências revogadas; verificar recuperação de recursos de CPU após encerramento. Memória ainda acessível por DMA permanece fora do conjunto reutilizável. |
| RQ-013 | Iniciar e encerrar aplicações independentes dos serviços incorporados. | O terminal executa um programa sem recompilar o kernel; erro de carregamento ou falha do programa não derruba os serviços; recursos elegíveis retornam ao gerenciador. |
| RQ-014 | Oferecer armazenamento persistente por serviços de usuário. | Criar, ler e alterar arquivo; reiniciar a máquina e conferir os bytes confirmados. Ensaiar interrupção e corrupção conforme garantias explícitas do formato e da API. |
| RQ-015 | Oferecer rede por API utilizável por aplicações, mantendo protocolos fora do kernel. | Aplicação independente comunica por contrato documentado; testar TCP sob perda/retransmissão, DHCP e DNS no escopo definido, além das regressões ICMP/UDP. |
| RQ-016 | Definir confiança e ciclo de vida de DMA; confinar dispositivos antes de admitir drivers DMA não confiáveis. | Demonstrar, nos cenários e dispositivos definidos, recusa de DMA fora do domínio e ausência de reutilização prematura de buffers após revogação/reset. Até lá, declarar driver confiável e conservar a política de quarentena. |
| RQ-017 | Ampliar recuperação além da rede, com dependências e resultados pendentes tratados. | Provocar falha nos serviços cobertos, informar clientes, recuperar ou encerrar segundo política documentada e rejeitar referências antigas. Definir separadamente o que ocorre se o supervisor falhar. |
| RQ-018 | Entregar instalação/boot, atualização e suporte em uma plataforma explicitamente delimitada. | Reconstruir a distribuição, atualizar e recuperar uma atualização interrompida; publicar matriz de suporte e testes dessa matriz. Uma nova plataforma exige evidência própria. |

## 4. Não objetivos da formalização e da etapa 0.3

- Substituir um SO de uso geral, oferecer desktop, GUI de aplicações, compatibilidade
  POSIX ou execução de programas de Linux/Windows.
- Implementar agora aplicações dinâmicas, filesystem, TCP/DHCP/DNS, IOMMU ou a
  recuperação de todos os serviços; esses trabalhos têm dependências no roadmap.
- Ampliar simultaneamente para SMP, outras arquiteturas, USB, Wi-Fi ou hardware
  físico. A plataforma virtual atual permanece a referência até decisão específica.
- Prometer tempo real, desempenho de produção, correção formal ou isolamento de
  DMA hostil a partir dos ensaios existentes.

## 5. Promoção de estágio

| Estágio | Condição de entrada |
|---|---|
| Pesquisa executável — atual | Base 0.2 identificada, evidências existentes disponíveis, requisitos permanentes preservados e limites publicados. |
| Pesquisa com serviços orientados a eventos | RQ-009 a RQ-011 aceitos, regressões RQ-005 a RQ-008 preservadas e novo contrato versionado quando necessário. Continua sendo um SO experimental. |
| Ambiente experimental de aplicações | RQ-012 e RQ-013 aceitos, com origem dos executáveis e política de memória/autoridade documentadas. Esse estágio não implica persistência ou produto utilizável completo. |
| SO mínimo utilizável no escopo declarado | Aceitação conjunta de memória, aplicações, espera por eventos, persistência, recuperação geral no escopo, rede para aplicações e entrega/atualização; modelo DMA explícito e confinamento obrigatório se houver drivers não confiáveis. Aplicam-se os cenários da seção 6 da avaliação por Tanenbaum. |

Nenhum rótulo é promovido apenas por aumentar a versão. O registro de aceitação
deve apontar os requisitos cobertos, falhas conhecidas, artefato verificado e
plataforma correspondente. O roadmap mantém abertas as escolhas ainda sem ADR;
este PRD não as transforma em implementação concluída.

## 6. Documentos de referência

- [README e operação da versão atual](../../README.md).
- [Arquitetura e fronteiras do microkernel](../ARQUITETURA.md).
- [ABI 0.2](../ABI-0.2.md) e [ADR de ciclo de vida](../ADR-002-CICLO-DE-VIDA.md).
- [Avaliação e critérios de evolução](../CRITERIOS-TANENBAUM.md).
- [Evidências da base 0.2](../../evidencias/RESUMO-VALIDACAO.md).
- [Roadmap e dependências](ROADMAP.md).
- [Especificação proposta da 0.3](ESPECIFICACAO-0.3.md) e
  [modelo de ameaças e limites](AMEACAS-E-LIMITES.md).
