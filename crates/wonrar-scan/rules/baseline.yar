// Original WonRaR baseline rules, version 2026.10.14.1.
// Original application material; application licensing remains pending.
// Copyright (c) 2026 WonRaR contributors.
// No imported third-party rule collection. Indicators require human interpretation.
rule WonRaR_Test_Sentinel {
    strings: $marker = "WONRAR-BENIGN-SCAN-TEST-2026" ascii
    condition: $marker
}
rule WonRaR_EICAR_Test_File {
    strings:
        $start = "X5O!P%@AP[4\\PZX54(P^)7CC)7}$" ascii
        $end = "EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*" ascii
    condition: $start at 0 and $end
}
rule WonRaR_PowerShell_Download_And_Execute {
    strings:
        $download1 = "DownloadString(" ascii nocase
        $download2 = "Invoke-WebRequest" ascii nocase
        $execute1 = "Invoke-Expression" ascii nocase
        $execute2 = "IEX(" ascii nocase
        $execute3 = "IEX (" ascii nocase
    condition: any of ($download*) and any of ($execute*)
}
rule WonRaR_Shell_Download_Pipe_Execution {
    strings:
        $download1 = "curl " ascii
        $download2 = "wget " ascii
        $execute1 = "| sh" ascii
        $execute2 = "| bash" ascii
    condition: any of ($download*) and any of ($execute*)
}
rule WonRaR_Python_Deserialization_With_Process_Call {
    strings:
        $pickle = "pickle.loads(" ascii
        $process1 = "os.system" ascii
        $process2 = "subprocess." ascii
    condition: $pickle and any of ($process*)
}
