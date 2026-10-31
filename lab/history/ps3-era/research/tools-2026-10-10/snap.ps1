param($Src,$Dst)
$i=[IO.File]::Open($Src,'Open','Read','ReadWrite,Delete'); $o=[IO.File]::Create($Dst); $i.CopyTo($o); $o.Close(); $i.Close()
