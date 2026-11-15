# Histórico do Raiz OS

## Organização do projeto sobre a base 0.2.0

- Ponto de entrada do projeto, requisitos identificados, roadmap e backlog.
- Proposta de especificação 0.3 para espera bloqueante, prazos e notificações.
- Modelo de ameaças, critérios de validação, contribuição e continuidade.
- Repositório Git local a partir desta importação, sem remoto configurado.
- Código, ABI 2 e imagem 0.2 preservados; nenhuma funcionalidade 0.3 implementada
  e nenhum novo teste de execução alegado por esta reorganização documental.

## 0.2.0 — supervisor e recuperação limitada

- Terceiro serviço em ring 3, responsável por política de parada/recuperação da rede.
- ABI 2 com referência de instância e geração, rejeição de referências antigas e
  origem de mensagem autenticada pelo kernel.
- Mecanismos de reinício/encerramento restritos ao supervisor; terminal usa IPC.
- Preparação da instância nova antes da substituição e purga de mensagens da antiga.
- Arena física e tabelas novas por encarnação de rede; quatro encarnações por boot.
- Modo manual e modo automático com limite e atraso; recuperação manual de serviço
  executável preso em laço.
- Reset do e1000 com DMA desligado durante configuração; PCI command restrito às
  duas configurações autorizadas; atributos de cache coerentes para MMIO/vídeo.
- Novo teste de recuperação em duas VMs; regressões anteriores preservadas.
- 26 testes no host e cinco boots aprovados na plataforma virtual declarada.

## 0.1.0 — base executável do microkernel

- Carregador UEFI, kernel próprio e dois serviços isolados: terminal e rede.
- Paginação, preempção, permissões estáticas e IPC.
- Driver e1000 e Ethernet/ARP/IPv4/ICMP/UDP mínimos.
- Construtor e verificador GPT/FAT32 em Rust.
- Testes de boot, rede, proteção de memória e preempção.
- Matriz de avaliação fundamentada nos conceitos de Tanenbaum.

Nenhuma versão deste histórico declara um sistema de uso geral completo.
