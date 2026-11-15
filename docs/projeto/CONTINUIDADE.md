# Continuar o projeto em outra sessão

Este arquivo acompanha a pasta completa `raiz-os`. Ele não substitui as fontes,
a especificação ou as evidências do projeto.

## Contexto que precisa sobreviver

- Objetivo do usuário: SO próprio em Rust, microkernel próprio, sem base em outro
  SO, entregue como um produto integrado com boot, terminal, rede e evolução para
  aplicações e persistência.
- A base executável é 0.2.0, ABI 2. A organização do projeto acrescenta documentos;
  não implementa 0.3.
- Há carregador UEFI, núcleo x86-64, três serviços em ring 3, IPC com filas
  limitadas, preempção, rede básica e recuperação limitada da rede.
- Referências identificam instâncias por geração; a autorização continua
  estática. Não há capabilities dinâmicas nem IOMMU.
- A base permite a instância inicial da rede e três reinícios por boot. Arenas
  antigas não são reutilizadas. Terminal e supervisor não são reiniciáveis.
- Os 26 testes no host e cinco boots constam da validação histórica da 0.2.
  Conferir os relatórios não equivale a repetir os testes.
- O uso de Tanenbaum está documentado como aplicação dos conceitos de abstração
  e administração de recursos; não há selo ou checklist universal do autor.

## Tarefa inicial da próxima evolução

Leia o item de contrato da 0.3 no [backlog](BACKLOG.csv) e a
[especificação proposta](ESPECIFICACAO-0.3.md). Feche estados de espera, resultados,
deadlines, cancelamento, fila cheia, autoridade sobre eventos e interação com
reinício. Os números e layouts de uma ABI futura precisam de decisão e código;
a proposta não muda o ABI vigente.

O piloto de IRQ é COM1/IRQ4 exclusiva no QEMU de referência. Interrupções da e1000
e PS/2 não fazem parte desse piloto. A integração deve permitir polling com
espera por prazo onde ainda não existir entrega de IRQ.

Implemente incrementos que possam ser revisados e validados. Uma entrega completa
do marco exige o fluxo sem perda de despertar, deadlines determinísticos,
cancelamento de espera, filas limitadas, idle seguro e integração dos serviços,
além das regressões aplicáveis da base 0.2. Todos esses cenários novos ainda estão
por executar.

## Texto para entregar a um agente de programação

```text
Continue o projeto Raiz OS que acompanha esta mensagem.

Leia AGENTS.md, PROJETO.md, README.md e a documentação em docs/projeto/.
A versão implementada é 0.2.0, ABI 2. Não a substitua por um projeto novo.
O objetivo é um SO original em Rust, com microkernel e produto integrado.

Trabalhe no próximo item do backlog para a versão 0.3: primeiro fechar o
contrato de espera bloqueante, deadlines e notificações; depois implementar
os mecanismos e integrar os serviços em incrementos verificáveis. Use a
especificação 0.3 como proposta, resolva detalhes rotineiros dentro desse
escopo e registre as decisões relevantes em uma ADR. Preserve as fronteiras
de autoridade, as gerações de instância e o limite de recuperação existente.

O piloto de IRQ é UART COM1/IRQ4 exclusiva. Não suponha IRQ da e1000 resolvida
nem declare ABI 3 antes de definir e implementar o contrato correspondente.
Preserve alterações de outras pessoas. Não publique para um remoto sem
autorização. Informe os testes realmente executados e os limites restantes.

Use as evidências existentes como base histórica. Recompile antes de testar
código novo no QEMU. Não transforme um teste de lógica no host em uma
alegação de isolamento por hardware ou proteção contra DMA.
```

O texto acima autoriza a próxima evolução quando usado como tarefa. Neste pacote,
ele é apenas um roteiro de continuidade; não registra execução da versão 0.3.
