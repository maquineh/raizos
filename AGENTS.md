# Instruções para continuar o Raiz OS

Estas instruções se aplicam ao repositório inteiro. Preserve o trabalho existente
e siga a tarefa e as prioridades expressas pelo usuário na sessão atual.

## Contexto e leitura inicial

- A base implementada é Raiz OS 0.2.0, ABI 2; o plano 0.3 é uma proposta.
- Comece por `PROJETO.md`, `README.md` e pelo item relevante de
  `docs/projeto/BACKLOG.csv`.
- Antes de mudar mecanismos do kernel, leia `docs/ARQUITETURA.md`, as ADRs
  pertinentes e `docs/ABI-0.2.md`.
- Consulte `docs/projeto/VALIDACAO-E-ENTREGA.md` e as evidências existentes.
  Resultados históricos não são testes executados na sessão atual.

## Decisões que orientam a implementação

1. O SO tem implementação própria em Rust. Não substitua o núcleo por Linux,
   BSD, Redox, seL4, MINIX ou outro sistema. Especificações e ferramentas de
   desenvolvimento não são uma base de código de SO.
2. Preserve o microkernel: mecanismos de isolamento, agendamento, IPC e acesso
   controlado a recursos ficam no núcleo. Protocolos de rede, interpretação de
   comandos, arquivos e política de recuperação pertencem aos serviços.
3. Preserve a entrega integrada por imagem e os espaços de endereço distintos.
   Uma única distribuição não exige um único processo privilegiado.
4. Use português nos novos nomes de domínio e na documentação, coerente com os
   módulos existentes. Preserve nomes padronizados de protocolos e ferramentas.
5. Mantenha os crates do convidado em `no_std`. As ferramentas do host podem usar
   `std`. A base não possui crates de terceiros; uma dependência nova precisa de
   justificativa técnica, origem e impacto explicitados na alteração.
6. Mantenha `unsafe`, MMIO e assembly em trechos pequenos, com precondições e
   invariantes identificáveis. Ser escrito em Rust não demonstra correção de
   hardware, isolamento DMA ou ausência de falhas.

## ABI, autoridade e ciclo de vida

- Mensagens e structs de ABI precisam de representação e tamanhos explícitos.
  Não envie referências Rust, ponteiros locais, `Vec`, `String` ou trait objects
  entre espaços de endereço como se fossem objetos compartilhados.
- Referências de instância com geração são identidades; não são capabilities.
  Verifique a autorização do chamador separadamente.
- Defina resultados para fila cheia, destino inexistente, geração antiga,
  recursos esgotados e estado inválido. Não silencie falhas relevantes.
- Ao alterar o ABI, atualize contrato, wrappers, kernel, serviços e documento
  correspondente. Não declare ABI 3 com base apenas no plano 0.3.
- Preserve a preparação de uma instância nova antes de substituir a atual e a
  invalidação de mensagens e referências da anterior.
- A base permite quatro encarnações da rede por boot. Não reutilize suas arenas
  em quarentena sem resolver ciclo de DMA e confinamento. Uma tabela de páginas
  de CPU não limita os endereços DMA do dispositivo.
- O supervisor decide a recuperação; o kernel impõe autoridade e mecanismos.
  Terminal e supervisor ainda não são reiniciáveis.

## Forma de trabalhar

Inspecione o estado do Git e os arquivos afetados antes de editar. Faça a menor
alteração coerente com a tarefa; preserve alterações de outras pessoas e agentes.
Distribua trabalho independente por arquivos ou componentes com dono claro.

Resolva escolhas rotineiras de implementação dentro do escopo autorizado e
documente as decisões. Uma especificação marcada como proposta não é uma ordem
para interromper a tarefa e pedir confirmação de cada detalhe. Mudanças que
alterem o objetivo do produto ou a arquitetura escolhida pelo usuário devem ser
explicadas antes de adotadas.

Novas decisões relevantes entram em uma ADR com contexto, escolha, alternativas,
consequências e estado. Atualize o backlog com evidências reais; distinguir
proposto, em andamento, implementado e validado é mais útil que percentuais sem
critério. Não invente testes, logs, benchmarks, suporte a hardware ou datas.

Não sobrescreva logs históricos para apresentar uma nova implementação como já
validada. Preserve a relação entre fonte, binário, ambiente, comandos e resultado.
Não publique ou envie alterações para um serviço externo sem autorização para a
ação. A organização local do projeto não cria, por si só, um repositório remoto.

## Construção e validação

Use a toolchain e os alvos de `rust-toolchain.toml`. Os comandos partem da raiz:

```sh
cargo fmt --all --check
cargo test -p protocolos -p ferramentas -p contrato-boot -p contrato-sistema
cargo test -p nucleo --lib
cargo raiz compilar
cargo raiz verificar-imagem
cargo raiz testar
cargo raiz testar-isolamento
cargo raiz testar-recuperacao
```

Escolha a parte da verificação que resolve o risco da alteração. Documentação
requer conferir links, exemplos e consistência; não exige novos boots. Mudanças
no kernel, ABI ou ciclo de vida exigem testes pertinentes e regressões em VM.
Não use `cargo test --workspace` como atalho: há binários bare metal que dependem
do processo de construção específico.

`executar` e `testar*` usam a imagem existente. Recompile antes de testar código
alterado. Configure `RAIZ_QEMU`, `RAIZ_OVMF` e `RAIZ_OVMF_VARS` para o ambiente
local, sem gravar caminhos privados ou credenciais no repositório.

Ao terminar, informe o que mudou, por quê, o que foi realmente verificado e quais
limites permanecem. Preserve a classificação de SO experimental de propósito
restrito até que os critérios de promoção estejam demonstrados.
