$ErrorActionPreference = 'Stop'
$raiz = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (-not $env:RAIZ_QEMU -or -not $env:RAIZ_OVMF -or -not $env:RAIZ_OVMF_VARS) {
    throw 'Defina RAIZ_QEMU, RAIZ_OVMF e RAIZ_OVMF_VARS antes de executar.'
}
$instante = [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fffffff')
$pasta = Join-Path $PSScriptRoot "ps2-$instante"
New-Item -ItemType Directory -Path $pasta | Out-Null
$serial = Join-Path $pasta 'serial.log'
$erros = Join-Path $pasta 'qemu-stderr.log'
$saida = Join-Path $pasta 'qemu-stdout.log'
$pastaVars = Join-Path $raiz 'target/preservacao-antes-rz021-20261115/ps2-vars'
New-Item -ItemType Directory -Force -Path $pastaVars | Out-Null
$vars = Join-Path $pastaVars "ps2-$instante.fd"
Copy-Item -LiteralPath $env:RAIZ_OVMF_VARS -Destination $vars
$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$listener.Start()
$porta = $listener.LocalEndpoint.Port
$listener.Stop()
function Aspas([string]$s) { '"' + $s.Replace('"', '\"') + '"' }
function LerSerial {
    if (-not (Test-Path $serial)) { return '' }
    $arquivo = [IO.File]::Open($serial, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
    $leitor = [IO.StreamReader]::new($arquivo)
    try { return $leitor.ReadToEnd() } finally { $leitor.Dispose() }
}
$argumentos = @(
    '-machine', 'pc', '-accel', 'tcg', '-m', '256M', '-smp', '1',
    '-drive', (Aspas "if=pflash,format=raw,unit=0,readonly=on,file=$env:RAIZ_OVMF"),
    '-drive', (Aspas "if=pflash,format=raw,unit=1,file=$vars"),
    '-drive', (Aspas "if=ide,format=raw,snapshot=on,file=$(Join-Path $raiz 'saida/raiz-os.img')"),
    '-device', 'e1000,netdev=rede,mac=52:54:00:12:34:56',
    '-netdev', 'user,id=rede,hostfwd=udp:127.0.0.1:17777-10.0.2.15:7777',
    '-serial', (Aspas "file:$serial"), '-monitor', "tcp:127.0.0.1:$porta,server=on,wait=off",
    '-display', 'none', '-no-reboot'
)
if ($env:RAIZ_QEMU_DADOS) { $argumentos += @('-L', (Aspas $env:RAIZ_QEMU_DADOS)) }
$processo = $null
$cliente = $null
try {
    $processo = Start-Process -FilePath $env:RAIZ_QEMU -ArgumentList $argumentos -WorkingDirectory $raiz -WindowStyle Hidden -RedirectStandardOutput $saida -RedirectStandardError $erros -PassThru
    $limite = [DateTime]::UtcNow.AddSeconds(40)
    do {
        if ($processo.HasExited) { throw 'QEMU terminou antes da prontidão.' }
        Start-Sleep -Milliseconds 100
        $texto = LerSerial
    } until (($texto.Contains('[teste] TERMINAL_PRONTO') -and $texto.Contains('[rede] SERVICO_PRONTO') -and $texto.Contains('[supervisor] SERVICO_PRONTO')) -or [DateTime]::UtcNow -ge $limite)
    if (-not $texto.Contains('[teste] TERMINAL_PRONTO') -or -not $texto.Contains('[rede] SERVICO_PRONTO') -or -not $texto.Contains('[supervisor] SERVICO_PRONTO')) { throw 'Prazo de prontidão esgotado.' }
    $inicio = $texto.Length
    $cliente = [Net.Sockets.TcpClient]::new('127.0.0.1', $porta)
    $fluxo = $cliente.GetStream()
    foreach ($tecla in @('s','o','b','r','e','ret')) {
        $bytes = [Text.Encoding]::ASCII.GetBytes("sendkey $tecla`n")
        $fluxo.Write($bytes, 0, $bytes.Length)
        $fluxo.Flush()
        Start-Sleep -Milliseconds 200
    }
    $limite = [DateTime]::UtcNow.AddSeconds(15)
    $esperado = 'Terminal PID=1 CPL=3 / rede PID=2 / supervisor PID=3 / nucleo CPL=0'
    do {
        Start-Sleep -Milliseconds 100
        $texto = LerSerial
        if ($processo.HasExited) { throw 'QEMU terminou antes da resposta.' }
    } until ($texto.Substring($inicio).Contains($esperado) -or [DateTime]::UtcNow -ge $limite)
    if (-not $texto.Substring($inicio).Contains($esperado)) { throw 'Resposta ao comando PS/2 ausente.' }
    foreach ($falha in @('FALHA_NUCLEO','SERVICO_FALHOU','SERVICO_FINALIZADO','IRQ4 indisponivel','Falha ao confirmar IRQ4','Falha na espera IRQ4','Evento inesperado; polling serial','Erros UART LSR=','panic','PANIC')) {
        if ($texto.Contains($falha)) { throw "Falha observada: $falha" }
    }
    [IO.File]::WriteAllText((Join-Path $pasta 'resultado.txt'), 'APROVADO: comando sobre via PS/2 e resposta distinta observados.')
    Write-Output "APROVADO: $pasta"
} catch {
    [IO.File]::WriteAllText((Join-Path $pasta 'resultado.txt'), "FALHOU: $_")
    throw
} finally {
    if ($cliente) { $cliente.Dispose() }
    if ($processo -and -not $processo.HasExited) { Stop-Process -Id $processo.Id -Force; $processo.WaitForExit() }
}
