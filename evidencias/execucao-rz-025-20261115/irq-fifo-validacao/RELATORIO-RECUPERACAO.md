# Recuperação de serviços — Raiz OS 0.2

Resultado geral: **APROVADO**.

Duração: 37.561 segundos.

Ambiente solicitado: QEMU/TCG, pc, 1 CPU, 256 MiB, OVMF e e1000. Dois boots independentes e sequenciais.

## Supervisor, referências e limite de recursos

Resultado: **APROVADO**.

- Imagem GPT/FAT32 e bytes de BOOTX64.EFI verificados.
- Boot: terminal, supervisor em CPL=3/manual/limite=4 e rede em CPL=3/geração 1 prontos.
- Geração 1, inicial: ping do gateway recebido e ecos UDP de 0/1/17/1472 bytes conferidos.
- Referência da geração 1 guardada antes da falha.
- UD2 encerrou a execução da rede na geração 1; terminal respondeu e observou rede=2 antes de solicitar reinício manual.
- Supervisor confirmou reinício manual da geração 1 para a 2; nova rede pronta e executável.
- Envio IPC e consulta de estado com referência da geração 1 foram recusados após a geração 2 iniciar.
- Chamadas diretas REINICIAR e ENCERRAR do terminal foram negadas; rede permaneceu ativa na geração 2.
- Geração 2, após reinício manual e negativas de autoridade: ping do gateway recebido e ecos UDP de 0/1/17/1472 bytes conferidos.
- Em modo automático, page fault da geração 2 foi seguido de reinício confirmado pelo supervisor para a geração 3, com terminal vivo.
- Geração 3, após recuperação automática: ping do gateway recebido e ecos UDP de 0/1/17/1472 bytes conferidos.
- parar-rede mudou a política para manual e deixou a geração 3 em estado finalizado (rede=3), com terminal responsivo.
- Supervisor retomou a rede por reinício manual da geração 3 para a geração 4.
- Geração 4, após parada e retomada: ping do gateway recebido e ecos UDP de 0/1/17/1472 bytes conferidos.
- Geração 4, após recusa da quinta encarnação: ping do gateway recebido e ecos UDP de 0/1/17/1472 bytes conferidos.
- Quinta encarnação recusada por RECURSOS; a geração 4 permaneceu ativa, respondeu a ping/UDP e o terminal continuou responsivo.
- Log do kernel conferido: 3 trocas, 4 arenas físicas distintas, encadeamento de arenas/referências preservado: 0x24ae000 → 0x26ae000 → 0x28ae000 → 0x2aae000.
- Exatamente um EXIT_BOOT_SERVICES_OK observado neste cenário; não houve novo boot no log capturado.

Respostas UDP idênticas ao envio verificadas: 20. Bytes de payload conferidos: 7450.

Logs: `serial-recuperacao.log` e `qemu-recuperacao-stderr.log`.

## Recuperação de rede em loop sem cessão voluntária

Resultado: **APROVADO**.

- Imagem GPT/FAT32 e bytes de BOOTX64.EFI verificados.
- Boot: terminal, supervisor em CPL=3/manual/limite=4 e rede em CPL=3/geração 1 prontos.
- Rede na geração 1 confirmou loop sem ceder; terminal continuou executando antes do reinício.
- Geração 2, após reinício da rede presa em loop: ping do gateway recebido e ecos UDP de 0/1/17/1472 bytes conferidos.
- Supervisor reiniciou o serviço ativo preso em loop para a geração 2; terminal e rede voltaram a responder às verificações.
- Log do kernel conferido: 1 trocas, 2 arenas físicas distintas, encadeamento de arenas/referências preservado: 0x24ae000 → 0x26ae000.
- Exatamente um EXIT_BOOT_SERVICES_OK observado neste cenário; não houve novo boot no log capturado.

Respostas UDP idênticas ao envio verificadas: 4. Bytes de payload conferidos: 1490.

Logs: `serial-recuperacao-loop.log` e `qemu-recuperacao-loop-stderr.log`.

## Alcance e limites

O ciclo principal exerce recuperação manual após instrução inválida, recuperação automática após page fault, recusa de referências de uma geração antiga, autoridade exclusiva do supervisor para reiniciar/encerrar, parada voluntária e retomada. Na quarta encarnação, solicita uma quinta e exige recusa por recursos sem perda da instância ativa. A rede precisa responder a ping e UDP após cada recuperação e também após a recusa.

O segundo boot testa o reinício de uma rede presa em loop sem cessão voluntária. Ele possui orçamento próprio de encarnações e não amplia artificialmente o orçamento do primeiro cenário.

Esses testes não comprovam contenção por IOMMU, proteção contra DMA malicioso, política de reinícios ilimitados, restauração de conexões TCP, persistência de estado de aplicações ou prontidão para produção. As arenas antigas não são recicladas no cenário testado; o limite de quatro encarnações é parte do comportamento observado.

Os emuladores são encerrados após cada caso, inclusive em erro. Os relatórios e logs dos comandos `testar` e `testar-isolamento` são preservados.
