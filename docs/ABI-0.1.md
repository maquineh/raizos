# ABI de sistema 0.1

Contrato canônico: `contrato-sistema/src/lib.rs`. Arquitetura x86-64 little-endian.
Chamadas por `int 0x80`, acessível de CPL 3. Registradores gerais são preservados
exceto os explicitamente usados para resultados. Não há ABI de ponto flutuante.

## Convenção

| Papel | Registrador |
|---|---|
| Número da chamada | RAX |
| Argumentos 1–6 | RDI, RSI, RDX, R10, R8, R9 |
| Resultado comum | RAX |

`CEDER` pode trocar o processo ativo; todos os processos também estão sujeitos
a preempção. As chamadas têm efeitos de memória; wrappers de assembly não podem
usar `pure` ou `nomem`.

## Chamadas

| Número | Operação | Entrada | Saída |
|---|---|---|---|
| 0 | CEDER | — | 0 após voltar a executar |
| 1 | ENVIAR | destino, código, a, b, c, d | 0 ou erro |
| 2 | RECEBER | — | RAX=código; RDI=origem; RSI=a; RDX=b; R10=c; R8=d |
| 3 | SAIR | — | Não retorna |
| 4 | PCI_LER | BDF, registro | Valor ou erro |
| 5 | PCI_COMANDO | BDF, valor | 0 ou erro |
| 6 | ESTADO_SERVICO | ID | 0 inexistente; 1 executável; 2 falho; 3 finalizado |
| 7 | TIQUES | — | Contador do timer |

`RECEBER` com RAX=0 indica fila vazia; código zero não é uma mensagem válida.
A origem é preenchida pelo núcleo. O cliente não pode escolher uma origem falsa.
Há 16 posições por destino, sem alocação dinâmica e sem transferência de ponteiros.
O syscall de envio admite o par terminal/rede configurado no bootstrap.

## Erros

| Valor u64 | Significado |
|---|---|
| `MAX-1` | Permissão recusada |
| `MAX-2` | Fila cheia |
| `MAX-3` | Serviço inexistente/indisponível |
| `MAX-4` | Chamada/operação inválida |

O serviço de rede é ID 2; o terminal, ID 1. IDs são estáticos nesta versão e não
são handles dinâmicos com proteção contra reutilização. A futura ABI deverá
adicionar geração e direitos por objeto antes de suportar reinício dinâmico.

## Eventos entre terminal e rede

| Código | Evento | Campos |
|---|---|---|
| 1 | REDE_PRONTA | a=0 sucesso/1 falha; b=MAC big-endian; c=IPv4 big-endian; d=CPL |
| 2 | PING_SOLICITAR | a=IPv4 |
| 3 | PING_OK | a=IPv4; b=sequência |
| 4 | REDE_STATUS_SOLICITAR | — |
| 5 | REDE_STATUS | a=RX; b=TX; c=ping_ok; d=udp_eco |
| 6 | REDE_FALHAR | Teste de instrução inválida no serviço |
| 7 | REDE_ERRO | a=código interno |
| 8 | REDE_OCUPAR | Teste de laço sem ceder |
| 9 | REDE_ACESSAR_NUCLEO | Teste de leitura proibida pela MMU |
| 10 | REDE_EM_LACO | Aviso imediatamente anterior ao laço de teste |

Este protocolo é específico do laboratório. Não é a futura interface geral de
sockets; não possui identificador de requisição, prazo ou múltiplos clientes.

## ABI de entrada do serviço

ELF64 ET_EXEC, entrada `_inicio_servico`, ABI System V, RDI apontando para
`InfoServico` em página readonly na VA `0x401ff000`. A pilha começa com alinhamento
de chamada SysV. Retornar da entrada é inválido; use `SAIR`.

| Região virtual | Uso |
|---|---|
| 0x40000000 em diante | Segmentos da imagem; validação de limites e W^X |
| 0x401df000–0x401e0000 | Página de guarda inferior, não mapeada |
| 0x401e0000–0x401fe000 | Pilha; extremo superior exclusivo |
| 0x401fe000–0x401ff000 | Página de guarda superior, não mapeada |
| 0x401ff000 | Informação somente leitura do serviço |
| 0x50000000 | MMIO e1000, somente para rede, 128 KiB |
| 0x60000000 | Framebuffer, somente para terminal, tamanho recebido no contrato |

As guardas são a página imediatamente abaixo de `INICIO_PILHA` e a página em
`TOPO_PILHA`. `InfoServico.base_fisica` corresponde à reserva do processo e é usado
pelo driver de rede para converter seus próprios buffers em endereços DMA.
`tamanho_imagem` registra a extensão dos segmentos carregados, não os 2 MiB
inteiros da reserva.

