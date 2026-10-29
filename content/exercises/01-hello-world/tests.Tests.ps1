BeforeAll {
    $learnerScript = '/workspace/learner.ps1'

    $script:learnerOutput = @(
        & pwsh -NoLogo -NoProfile -NonInteractive `
            -File $learnerScript 2>&1
    )

    $script:learnerExitCode = $LASTEXITCODE
    $script:learnerText = (
        $script:learnerOutput | Out-String
    ).Trim()
}

Describe 'Say hello exercise' {
    It 'finishes successfully' {
        $script:learnerExitCode | Should -Be 0
    }

    It 'prints the required greeting' {
        $script:learnerText | Should -Be 'Hello, world!'
    }
}