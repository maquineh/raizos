# Revisão da organização do projeto Raiz OS

**Escopo:** formalização do projeto sobre a base executável 0.2.0, ABI 2.
Esta revisão acrescenta requisitos, roadmap, backlog, proposta técnica 0.3,
instruções de contribuição e continuidade. Não implementa funcionalidades do
próximo marco.

## Verificações realizadas nesta organização

| Verificação | Resultado |
|---|---|
| Preservação das fontes, configurações e scripts de ligação | 44 arquivos técnicos comparados por SHA-256 com a base anterior, sem alteração |
| Preservação da imagem e do carregador EFI | 2 artefatos comparados por SHA-256, sem alteração |
| Links locais em Markdown | 26 documentos e 144 referências locais conferidos, sem destino ausente |
| Backlog CSV | 40 tarefas, 10 colunas, leitura UTF-8 válida e IDs únicos |
| Rastreabilidade de requisitos | Todas as tarefas ligadas a requisitos; RQ-001 a RQ-018 cobertos |
| Dependências de tarefas | Todos os IDs existem e o grafo não contém ciclos |
| Status do backlog | 12 entregas históricas 0.2 concluídas; 28 propostas; nenhuma implementação 0.3 concluída |
| Evidências citadas pelas tarefas concluídas | Todos os arquivos indicados existem |
| Revisão de conteúdo | Consistência entre escopo, requisitos, roadmap, contrato vigente e proposta 0.3 revisada |

Os casos T03-01 a T03-25 são critérios futuros da
[especificação 0.3](../docs/projeto/ESPECIFICACAO-0.3.md). Nenhum foi executado
nesta formalização. O piloto de IRQ proposto permanece COM1/IRQ4 exclusiva;
rede e PS/2 conservam polling por prazo no recorte planejado.

## Binários preservados

| Artefato | SHA-256 conferido |
|---|---|
| `saida/raiz-os.img` | `e3d637664af0a65af430b0696db1f522540bd21a72a5edc178980f367779e60e` |
| `saida/esp/EFI/BOOT/BOOTX64.EFI` | `b6d949e6fa4bd4f7cb007018818d3dcce5e567336eb26caeff356dbb45b81947` |

Os 26 testes no host, a compilação e os cinco boots continuam sendo evidência
histórica de 0.2. Não foram executados novamente para esta mudança documental.
Veja o [resumo da validação original](RESUMO-VALIDACAO.md).

O manifesto [SHA256SUMS](SHA256SUMS) é atualizado para os arquivos deste pacote,
incluindo a documentação acrescentada e os binários preservados. Ele não inclui
a si mesmo, dados internos do Git, cache de compilação ou estado mutável da VM.
A conferência de integridade não constitui uma nova validação funcional do SO.

## Continuidade

O [ponto de entrada do projeto](../PROJETO.md) reúne os documentos e explica o
próximo trabalho. O [backlog](../docs/projeto/BACKLOG.csv) identifica RZ-013 como
a revisão e decisão do contrato de espera do marco 0.3; sua proposta disponível
não modifica a ABI 2 atualmente implementada.
