# This is a vhs script. See https://github.com/charmbracelet/vhs for more info.
# To run this script, install vhs and run `vhs hangman.tape`
Output "hangman.gif"
Set Theme "Aardvark Blue"
Set FontSize 12
Set Width 628
Set Height 350
Set Padding 0
Hide
Type "cargo run -- -w elephant -H Animal"
Enter
Sleep 2s
Show
Sleep 2s
Type E
Sleep 2s
Type T
Sleep 2s
Type R
Sleep 2s
Type S
Sleep 2s
Type A
Sleep 2s
Type L
Sleep 2s
Type H
Sleep 2s
Type N
Sleep 2s
Type P
Sleep 2s
