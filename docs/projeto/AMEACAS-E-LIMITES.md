# Ameaças, fronteiras de confiança e limites

**Base:** Raiz OS 0.2.0 e evidências preservadas no projeto.  
**Finalidade:** definir o que a arquitetura protege no escopo atual, o que ainda depende de confiança e quais condições bloqueiam sua expansão.  
**Estado:** modelo de ameaças inicial, revisável. Não é certificação, auditoria completa ou declaração de prontidão para produção.

As propostas da [especificação 0.3](ESPECIFICACAO-0.3.md) não estão implementadas apenas por constarem neste documento. Os requisitos RQ-005, RQ-006, RQ-007, RQ-010, RQ-011, RQ-012, RQ-016 e RQ-017 orientam as fronteiras discutidas aqui.

## 1. Escopo real da proteção

A versão 0.2 contém kernel próprio em Rust, inicializador UEFI próprio e três processos em CPL 3: terminal, rede e supervisor. A proteção de CPU usa espaços de endereçamento separados, páginas de usuário com permissões e acesso de hardware atribuído. O kernel aplica política estática de IPC e de gerenciamento. Esses mecanismos constam em [`memoria.rs`](../../nucleo/src/memoria.rs), [`cpu.rs`](../../nucleo/src/cpu.rs), [`controle.rs`](../../nucleo/src/controle.rs) e [`plataforma.rs`](../../nucleo/src/plataforma.rs).

A plataforma efetivamente ensaiada foi QEMU `pc`/i440fx, TCG, uma CPU, 256 MiB, OVMF e e1000 emulada. O [resumo de validação](../../evidencias/RESUMO-VALIDACAO.md) registra 26 testes no host e cinco boots de integração. Esses resultados demonstram casos concretos de contenção de falha e recuperação nessa plataforma; não demonstram resistência geral a serviços maliciosos, DMA arbitrário, falhas físicas ou firmware comprometido.

O produto é independente de Linux como kernel ou sistema-base de execução. Isso não significa independência do firmware da máquina, do compilador, do linker, das bibliotecas do toolchain, do ambiente de build ou do emulador usado para testar. UEFI é uma fronteira de inicialização e os formatos/protocolos usados são interfaces externas. [1]

## 2. Ativos e fronteiras

| Ativo | Fronteira relevante | O que precisa permanecer verdadeiro |
|---|---|---|
| Código e dados do kernel | CPL 0, paginação e entrada de exceções/syscalls | Um acesso comum de CPU de um serviço não pode modificar o núcleo |
| Memória de cada serviço | Mapeamento virtual próprio e permissões de página | A memória de outra instância não pode ser acessada por CPU apenas por conhecer seu endereço |
| Identidade de instância | Referência com geração | Uma operação da geração antiga não pode atingir silenciosamente a sucessora |
| Autoridade de gerenciamento | Identidade real do chamador e política no kernel | Só o supervisor administra o alvo permitido; conhecer a referência não basta |
| Mensagens pendentes | Filas limitadas, origem preenchida pelo kernel e purga seletiva | Conteúdo e ordem de mensagens preservadas não podem ser confundidos com a origem revogada |
| Dispositivo e buffers DMA | PCI/MMIO, bus mastering, memória física e futuro IOMMU | Enquanto não houver confinamento DMA, o driver que programa a NIC continua confiável |
| Disponibilidade do terminal e supervisor | Preempção, quotas e política de recuperação | Falha da rede não deve exigir reboot nos casos de recuperação aceitos |
| Imagem e evidência de execução | Build, integridade dos arquivos e harness | O relatório deve identificar o artefato e o cenário ensaiados; não atribuir teste a código diferente |

Dados de usuários, identidades de login, arquivos persistentes e sessões TCP não formam hoje um domínio completo de proteção porque esses subsistemas ainda não existem. Isso é ausência de funcionalidade, não segurança obtida por sua implementação.

## 3. Quem precisa ser confiável hoje

### Kernel e inicializador

O kernel determina páginas, frames de retorno, referências, filas e permissões. Um erro nele pode comprometer todos os serviços. O inicializador determina a imagem carregada e o contrato de boot. Usar Rust não elimina falhas lógicas, erros de ABI, acessos `unsafe`, Assembly, programação incorreta de registradores ou pressupostos errados de hardware.

Os alvos Rust usados permitem executáveis UEFI e bare-metal; eles não certificam os invariantes do sistema que está sendo construído. [2] As fronteiras `unsafe` e os pontos de entrada de CPU precisam de revisão específica, além de compilação sem warnings.

### Firmware, hardware e ferramentas

Firmware, microcódigo, plataforma e ambiente de execução estão fora da proteção que esse kernel fornece. Não há evidência de cadeia de boot autenticada, medição de firmware, proteção contra SMM hostil, atestação ou isolamento de um host QEMU malicioso.

O compilador/linker e os componentes que produzem a imagem são confiáveis para esta etapa. Fixar versões e guardar hashes ajuda a reproduzir e identificar resultados; não prova ausência de comprometimento da cadeia de ferramentas. Um SHA-256 calculado sobre o artefato não autentica por si só quem o produziu. A versão 0.2 não implementa Secure Boot ou atualização assinada como política própria do produto. [3]

### Serviços com hardware atribuído

Rede e terminal têm permissões de dispositivo. A rede programa DMA por MMIO; o terminal acessa portas serial/PS/2 e framebuffer. A limitação de páginas de CPU não impede todos os efeitos desses dispositivos. Por isso, a experiência de laboratório não deve tratar qualquer binário colocado nesses lugares como plugin hostil seguramente confinado.

O supervisor recebe autoridade limitada sobre o ciclo de vida da rede. Não recebe, por isso, acesso geral ao hardware, a todos os espaços de memória ou a uma operação irrestrita de criação de processos. A política é estática e pequena; um sistema futuro com vários usuários e aplicações exigirá outro modelo de concessão e revogação de direitos.

## 4. DMA: o limite mais importante da isolação atual

O teste de page fault da rede verifica acesso **de CPU** a uma página supervisor. Uma placa com bus mastering usa endereços físicos programados pelo driver. Sem IOMMU configurada e validada, a paginação do processo não fornece uma fronteira equivalente para esse acesso. O driver que controla a NIC pode programar descritores apontando para RAM fora de sua arena. A versão 0.2 não impede esse comportamento adversarial.

Há mitigação de um risco diferente: cada encarnação da rede recebe uma arena nova, e as anteriores não são reutilizadas durante o boot. O kernel desabilita bus mastering na revogação; a nova instância reseta o dispositivo e programa descritores antes de habilitá-lo. Isso reduz o risco de que atividade legítima tardia de uma encarnação anterior atinja uma área recém-reaproveitada. **Não demonstra drenagem universal de DMA nem protege contra um driver malicioso.** [4]

O controlador pode preservar parte do estado de configuração ao resetar; o projeto reprograma os registradores necessários. A conclusão de reset em QEMU não equivale a uma prova de comportamento em todas as placas físicas. [5]

### Gate obrigatório antes de ampliar a promessa de confiança

Não admitir driver DMA não confiável, reciclar suas arenas nem declarar isolamento completo de dispositivos antes de demonstrar, na plataforma escolhida:

1. Domínio IOMMU associado ao dispositivo correto e mapeamento apenas dos buffers permitidos.
2. Recusa observável de DMA fora do domínio, com tratamento de falha que preserve o kernel.
3. Ordem validada de máscara de IRQ, interrupção de novas transações, invalidação de tradução e revogação/reuso de memória.
4. Ausência de caminhos alternativos de DMA fora desse domínio no escopo reivindicado.
5. Recuperação/reassociação sem entregar a nova geração buffers ou notificações da antiga.

Esses critérios ainda não foram executados. O posicionamento de IOMMU em um marco futuro, por exemplo 0.7, não concede uma exceção ao gate. Se a separação NIC/pilha em 0.6 introduzir um driver considerado hostil ou reuso de memória DMA, o trabalho de confinamento precisa ser antecipado. Separar NIC e protocolos em dois processos reduz dependências de CPU, mas não resolve, sozinho, o acesso físico do driver.

Suporte a uma placa física deve começar em bancada com escopo e confiança explicitamente declarados. Até que esse gate seja satisfeito, tal piloto não deve ser apresentado como expansão da segurança para drivers não confiáveis.

## 5. Referências não são capabilities

A referência de 64 bits identifica ID e geração. Seu valor é previsível e não é segredo, credencial, prova de posse autorizada ou token criptográfico. A autorização continua sendo uma decisão separada do kernel. [`contrato-sistema`](../../contrato-sistema/src/lib.rs) e [ABI 0.2](../ABI-0.2.md) expressam essa distinção.

O kernel preenche a origem do IPC; o remetente não pode escolher esse campo pela syscall. As arestas permitidas são terminal↔rede e terminal↔supervisor. Reiniciar e encerrar a rede são operações restritas ao supervisor. O terminal pode **pedir** essas ações por seu protocolo autorizado, o que é uma autoridade de produto intencional, não um desvio da restrição de syscall.

O mecanismo de geração reduz confusão entre encarnações. Não oferece delegação de direitos, revogação de capabilities transferíveis, controle por usuário ou isolamento entre aplicações de vários proprietários. Ao introduzir esses requisitos, o projeto precisa decidir o modelo de objetos e concessões; não basta renomear referências como capabilities.

Um resultado aceito na fila também não equivale a uma transação concluída. A purga pode remover mensagens ainda pendentes de uma geração revogada. Mensagens já consumidas não são desfeitas; a aplicação deve correlacionar pedidos e respostas e definir quando considera uma operação concluída.

## 6. Ameaças e respostas atuais

| Cenário | Resposta existente ou limite explícito | Lacuna/gate |
|---|---|---|
| Serviço executa instrução inválida | Caso UD2 encerra a rede e preserva o terminal na integração | Não prova contenção de qualquer defeito em kernel, dispositivo ou firmware |
| Rede tenta ler página do kernel por CPU | Teste observa page fault vetor 14 e continuidade do terminal | Uma leitura ensaiada não é prova formal de todos os mapeamentos; DMA é outra fronteira |
| Serviço roda sem ceder | PIT preempta no cenário ensaiado; supervisor pode reiniciar a rede a pedido | Sem garantia de latência máxima e sem detecção automática de ausência de progresso |
| Chamador usa referência antiga | Envio/consulta recusados; controle verifica geração exata | Novas esperas/IRQs de 0.3 devem guardar geração e token próprios |
| Terminal tenta reiniciar diretamente | Kernel recusa autoridade; teste de recuperação observa a negativa | O supervisor não pode virar um encaminhador genérico de operações privilegiadas |
| Uma origem enche a fila | Capacidade fixa e recusa explícita impedem sobrescrita | Justiça de admissão e notificações reservadas são trabalho proposto de 0.3 |
| Reinício encontra quota esgotada | Quota é verificada antes de revogar; teste mantém rede viva na quarta geração | Não há recuperação ilimitada ou reuso seguro das arenas |
| Dispositivo continua usando buffer antigo | Arena permanece em quarentena; bus mastering é desabilitado na revogação | Drenagem completa e confinamento de DMA não demonstrados |
| Pacote malformado chega à pilha | Parsers verificam comprimentos/checksums e recusam recursos não suportados | Sem campanha ampla de fuzzing ou prova de resistência a tráfego adversarial sustentado |
| Imagem de boot é substituída | Ferramentas verificam formato e hashes do artefato produzido | Não há autenticação própria de boot/atualização nem verificação externa de proveniência |
| Terminal ou supervisor falha | Kernel registra falha; outros serviços podem continuar conforme o estado | Não há recuperação desses dois serviços nem canal administrativo alternativo garantido |
| Todas as threads ficam sem trabalho | Na 0.2 os serviços consultam/cedem; não existe espera bloqueante normal | Idle com eventos e tratamento de CPL 0 são trabalho proposto, não proteção atual |

As respostas observadas estão nos relatórios de [smoke](../../evidencias/RELATORIO-SMOKE.md), [isolamento](../../evidencias/RELATORIO-ISOLAMENTO.md) e [recuperação](../../evidencias/RELATORIO-RECUPERACAO.md). Onde a tabela descreve comportamento de código sem ensaio equivalente, isso não deve ser convertido em afirmação de teste realizado.

## 7. Recuperação limitada não é continuidade de aplicações

A rede admite quatro encarnações por boot, incluindo a original. O supervisor inicia manual; o modo automático observa falha/término, com orçamento e backoff. Uma rede executável presa em laço requer pedido explícito. Terminal e supervisor não são reiniciáveis nessa etapa. [4]

O reinício reconstrói contexto, páginas e driver, mas perde estatísticas, cache ARP e ping pendente. Os testes verificam que novos pings e novos datagramas UDP funcionam após a recuperação. Não demonstram continuidade de pedidos que estavam em andamento.

Não existe TCP na 0.2: não há conexões, números de sequência TCP, janelas, retransmissões ou estado de sessão a restaurar. Também não há filesystem persistente de execução do sistema, journal, transação de arquivo, processo `fs` ou recuperação de operações em disco. O FAT32 utilizado para a imagem de inicialização não é evidência de um serviço de arquivos persistentes do SO após o boot. [3]

Consequentemente, não anunciar reinício transparente de aplicações, operações exatamente uma vez, preservação de conexões ou tolerância a perda de energia. Esses objetivos precisam de protocolos de aplicação, persistência e critérios próprios quando os subsistemas existirem.

## 8. Riscos que a proposta 0.3 deve tratar

Introduzir bloqueio cria novos caminhos de falha: perder um despertar, concluir uma espera duas vezes, escrever no contexto de uma geração substituída, reconhecer IRQ de outro proprietário ou deixar todos os serviços bloqueados sem caminho de saída. Esses riscos são específicos da proposta e não foram cobertos pela execução 0.2.

As exigências mínimas são: registro/verificação atômicos na CPU única; referência e sequência por espera; remoção de prazos/inscrições ao revogar; conclusão única; limites de recursos; notificação privilegiada distinta de mensagem de usuário; autoridade de IRQ validada; e idle com entrada/retorno de contexto corretos. A [matriz T03](ESPECIFICACAO-0.3.md#13-matriz-de-testes-futuros--nenhum-executado-neste-documento) especifica cenários futuros, ainda não executados.

O piloto UART/IRQ4 não autoriza liberar todos os IRQs PCI. Uma tempestade de interrupções e um driver que não reconhece sua fonte podem afetar disponibilidade; máscara, orçamento e sequência de rearme precisam conservar o PIT e os outros serviços. A ampliação para linha compartilhada ou NIC exige ensaio separado de roteamento e tratamento da causa.

SMP não deve ser ativado com a justificativa de que o estado era protegido por `IF=0`: essa exclusão vale para a CPU única do desenho atual. Da mesma forma, permitir FPU/SIMD em aplicações exige tratamento do estado arquitetural que a configuração atual deliberadamente não oferece como ambiente geral.

## 9. O que pode ser afirmado e o que precisa continuar qualificado

| Afirmação | Situação |
|---|---|
| Há um microkernel próprio, com serviços em ring 3 | Sustentada pelo código e pelos casos de integração declarados |
| O protótipo contém uma falha de CPU da rede nos casos ensaiados | Sustentada por UD2 e page fault observados |
| A rede pode ser reiniciada sem reboot, dentro da quota | Sustentada pelos cenários de recuperação 0.2 |
| Uma referência antiga é recusada nas operações exercitadas | Sustentada pelos testes de identidade e integração |
| Todos os serviços maliciosos estão confinados | Não sustentada, especialmente para hardware/DMA atribuído |
| Rust torna o kernel automaticamente seguro | Não sustentada |
| Quarentena e reset equivalem a IOMMU | Incorreta |
| Interrupções, bloqueio e idle 0.3 já funcionam | Não implementados por esta documentação |
| O sistema recupera conexões e arquivos | Subsistemas e garantias correspondentes ausentes |
| A plataforma é adequada a produção ou hardware arbitrário | Não demonstrada |

Qualificar uma garantia não impede avançar. Define qual teste ou mecanismo precisa existir antes de usá-la como requisito atendido.

## 10. Evidência exigida antes de ampliar o escopo

Cada nova fronteira deve produzir: ameaça concreta, direito concedido, invariantes, caminhos de erro, teste negativo e resultado observável. O relatório deve vincular a execução ao código e à imagem, informar plataforma e limites e distinguir teste no host de teste no dispositivo/emulador.

Para hardware não confiável ou reuso de DMA, vale o gate da seção 4, independentemente do número do marco. Para novas aplicações/usuários, decidir concessão e revogação de direitos antes de oferecer interfaces extensíveis. Para recuperação persistente, definir primeiro o ponto de commit e como pedidos repetidos são reconhecidos. Para SMP, validar concorrência antes de reutilizar os argumentos de correção de CPU única.

Esta revisão documental não executou novos testes, não mudou o modelo de permissão do código e não transformou uma proposta em garantia aceita.

## Referências

1. UEFI, serviços de boot e transição de posse em `ExitBootServices`: <https://uefi.org/specs/UEFI/2.10/07_Services_Boot_Services.html>. Código do [inicializador](../../inicializador/src/main.rs).
2. Rust, alvos [x86_64-unknown-none](https://doc.rust-lang.org/rustc/platform-support/x86_64-unknown-none.html) e [unknown-uefi](https://doc.rust-lang.org/rustc/platform-support/unknown-uefi.html); [configuração da toolchain](../../rust-toolchain.toml).
3. [Formato da imagem](../FORMATO-IMAGEM.md), [compilação 0.2](../../evidencias/compilacao-0.2.log) e [resumo de validação](../../evidencias/RESUMO-VALIDACAO.md).
4. [ADR 002 — ciclo de vida](../ADR-002-CICLO-DE-VIDA.md), [mecanismo no kernel](../../nucleo/src/processos.rs), [mapeamentos e arenas](../../nucleo/src/memoria.rs) e [driver](../../servicos/rede/src/driver.rs).
5. Intel, *PCI/PCI-X Family of Gigabit Ethernet Controllers Software Developer's Manual*, revisão 4.0, reset e endereçamento DMA: <https://www.intel.com/content/dam/doc/manual/pci-pci-x-family-gbe-controllers-software-dev-manual.pdf>.
