# Testes de memória e preempção do Raiz OS

Resultado geral: **APROVADO**.

Duração: 13.288 segundos.

Ambiente solicitado: duas inicializações independentes e sequenciais em QEMU/TCG, pc, 1 CPU, 256 MiB, OVMF e e1000.

## Proteção de memória

Resultado: **APROVADO**.

- Imagem GPT/FAT32 e bytes de BOOTX64.EFI verificados antes do boot.
- Nova inicialização: ExitBootServices, terminal e serviço de rede em CPL=3 observados.
- Leitura de 0x02000000 pelo serviço de rede gerou page fault id=2, vetor=14, CR2=0x2000000.
- Terminal respondeu ao eco após o page fault; estados terminal=1 rede=2, tiques=308.

Logs: `serial-isolamento-memoria.log` e `qemu-isolamento-memoria-stderr.log`.

## Preempção

Resultado: **APROVADO**.

- Imagem GPT/FAT32 e bytes de BOOTX64.EFI verificados antes do boot.
- Nova inicialização: ExitBootServices, terminal e serviço de rede em CPL=3 observados.
- Serviço de rede confirmou entrada no loop sem cessão voluntária.
- Terminal respondeu com linha exata PREEMPCAO_OK enquanto a rede permanecia no loop.
- Primeira consulta: terminal=1 rede=1 tiques=217.
- Segunda consulta após 200 ms no host: terminal=1 rede=1 tiques=288; aumento de 71 tiques.

Logs: `serial-isolamento-preempcao.log` e `qemu-isolamento-preempcao-stderr.log`.

## Alcance da evidência

O caso de memória tenta uma leitura de CPU do endereço 0x02000000 pelo serviço de rede. Exige page fault do serviço 2 (vetor 14), CR2=0x2000000 e continuidade do terminal. Ele não testa acesso por DMA nem configura IOMMU.

O caso de preempção coloca a rede em loop sem chamadas voluntárias de cessão. Exige resposta do terminal, os dois serviços ativos e aumento dos tiques entre duas consultas separadas por pelo menos 200 ms no host. Isso evidencia a preempção nesse cenário; não é uma medição de latência máxima ou garantia de tempo real.

Os emuladores são encerrados após cada caso, inclusive em erro. Estes arquivos não substituem os logs e o relatório do comando `testar`. Os resultados não afirmam reinício automático, cobertura de todos os acessos de memória, contenção de dispositivos ou prontidão para produção.
