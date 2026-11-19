# Checkpoint de continuidade — 15/11/2026

## Objetivo e forma de trabalho

Continuar os itens RZ em ordem e, depois, evoluir a interface de janelas, áudio e ferramentas do sistema, com teclado, mouse e rede. Raiz OS continua sendo um sistema próprio em Rust, microkernel, experimental e de propósito restrito. O usuário pediu Sol 6.1 como orquestrador e Luna como desenvolvedor, com testes pertinentes em QEMU. Preservar as instruções AGENTS.md fornecidas na conversa; ABI corrente é 2 e não se deve declarar ABI 3 ou marco 0.3 concluído apenas por estar proposto.

## Estado do repositório

O checkout contém alterações e evidências não commitadas de várias etapas. Não restaurar, limpar, fazer stash ou sobrescrever trabalho existente. Este checkpoint é um arquivo local, não um commit. O estado Git no momento foi registrado em `checkpoint-20261115-git-status.txt`; os binários e backups em `target` não são garantidos por Git.

## Entregas e evidências

- RZ-022: serviços adaptados ao contrato de espera; conclusão e limites em [resumo RZ-022](../execucao-rz-022-20261115/RESUMO.md).
- RZ-023 e RZ-024 concluídos nos escopos registrados em [RZ-023](../execucao-rz-023-20261115/RESUMO.md) e [RZ-024](../execucao-rz-024-20261115/RESUMO.md). A [matriz T03](../execucao-rz-023-20261115/MATRIZ.md) apresenta as evidências e limites de cada cenário.
- RZ-025 permanece em andamento. Seu [resumo](RESUMO.md) registra preparação recusada, IRQ com FIFO cheia, UART, comparação descritiva com 0.2, empate de prazo e outros ensaios, sempre com limitações explícitas.
- T03-12 foi aprovado em duas execuções QEMU: [primeira](../teste-rx-quota-1794751363479086500/resultado.txt) e [segunda](../teste-rx-quota-1794751379029899500/resultado.txt). Quota por origem recusou ID 16 com 15 IPCs de rede; supervisor ocupou a 16ª posição e o segundo controle foi recusado; ID 16 foi reenviado e IDs 17–18 processados. Fluxo finito e ordenado em QEMU, sem garantia de ordem UDP, justiça ilimitada ou hardware físico. Tentativas anteriores de parser/barreira foram preservadas.
- Após T03-12 passaram fmt, 44 testes de pacotes, 35 testes do núcleo, build, verificação da imagem e cinco boots normais QEMU (smoke, dois isolamento, dois recuperação). Logs e hashes estão em [rx-quota-validacao](rx-quota-validacao/); os 13 arquivos históricos foram restaurados. Estes resultados antecedem as alterações posteriores da fixture T03-09, portanto não são regressões finais sobre o estado atual.
- Imagens e EFI do ensaio T03-12 foram preservados localmente em `target/preservacao-antes-rz022-20261115/artefatos-rx-quota-rz025/` com [hashes](rx-quota-validacao/artefatos-preservados.sha256). Os cinco stderr históricos não são novos resultados de QEMU.

## Trabalho interrompido: T03-09

A auditoria identificou que o teste histórico recusava a quinta instância antes de `Rede::inicializar`; ping/UDP posteriores não provam preservação de driver previamente ativo nem de filas. A fixture opt-in `teste-ciclo-espera` foi modificada para inicializar o driver primeiro, trocar snapshots de referência, endereço MMIO, arena e estado de software via IPC com ACK, e depois comparar o snapshot da geração 4 após a recusa. Não são feitas leituras MMIO/descritores; não se demonstra confinamento DMA ou ausência de reset transitório. Não há sonda de filas. O caminho normal não foi intencionalmente alterado.

O último estado passou, segundo o desenvolvedor, 35 testes de `ferramentas`, fmt, diff-check, build normal e `cargo raiz compilar --teste-ciclo-espera`. O último QEMU **falhou** em [1794753016432515800](../teste-ciclo-espera-1794753016432515800/resultado.txt): `Snapshot DRIVER_ATIVO gen4 ausente, duplicado ou malformado`. O [serial](../teste-ciclo-espera-1794753016432515800/serial.log) contém `DRIVER_ATIVO` gen4, `BLOQUEADA prazo=2406`, `RECURSOS_ESGOTADOS`, timeout no tick 2406 e `DRIVER_PRESERVADO igual=1`. A causa precisa do erro do parser ainda não foi determinada; não classificá-lo como aprovação. Dois ensaios anteriores com falha preservados: [primeiro](../teste-ciclo-espera-1794752431397843200/resultado.txt), prazo de inicialização confundido com espera-alvo; [segundo](../teste-ciclo-espera-1794752766992341000/resultado.txt), harness aguardou prontidão na ordem anterior.

## Retomada

1. Investigar o parser `validar_snapshot_driver_gen4` e o serial da última falha; corrigir a causa real sem relaxar verificação, acrescentar teste host e reconstruir a imagem opt-in antes de retestar em QEMU. Confirmar a ordem e a geração dos snapshots, prazo, recusa, timeout e resposta do supervisor. Executar também ping/UDP previstos pelo harness.
2. Se o cenário for aprovado, repetir conforme necessário, registrar fontes/artefatos/hashes e limites; compilar imagem normal e executar regressões pertinentes, preservando os 13 logs/relatórios históricos. Atualizar matriz/backlog com evidências reais, sem declarar RZ-025 concluído antes dos critérios.
3. Continuar as demais RZ em ordem e, depois, a interface gráfica e os demais recursos solicitados.

O ambiente local de QEMU/OVMF é configurado por `RAIZ_QEMU`, `RAIZ_QEMU_DADOS`, `RAIZ_OVMF` e `RAIZ_OVMF_VARS`; não gravar caminhos privados desses binários no repositório. Um worktree de comparação 0.2 criado pelo app permanece anexado: tentativa de arquivamento foi recusada pelo app. Não removê-lo manualmente.
