# SPDX-License-Identifier: GPL-3.0-or-later
# Bash completion for the Rust CLI. No screenshot or daemon is started.
_flameshot() {
    local cur="${COMP_WORDS[COMP_CWORD]}" prev="${COMP_WORDS[COMP_CWORD-1]}"
    local cmd="${COMP_WORDS[1]-}" options item help
    COMPREPLY=()
    if (( COMP_CWORD == 1 )); then
        options='gui screen full config launcher help --help -h --version -v -V'
        help=$(command flameshot --help 2>/dev/null)
        [[ $help == *$'\n  history '* ]] && options+=' history'
        [[ $help == *$'\n  updates '* ]] && options+=' updates'
        mapfile -t COMPREPLY < <(compgen -W "$options" -- "$cur")
        return
    fi
    for item in "${COMP_WORDS[@]:1:COMP_CWORD-1}"; do
        [[ $item == -- ]] && return
    done
    case "$cmd:$prev" in
        gui:-p|gui:--path|screen:-p|screen:--path|full:-p|full:--path)
            mapfile -t COMPREPLY < <(compgen -f -- "$cur")
            compopt -o filenames 2>/dev/null || :
            return ;;
        config:-a|config:--autostart|config:-n|config:--notifications|config:-t|config:--trayicon|config:-s|config:--showhelp)
            mapfile -t COMPREPLY < <(compgen -W 'true false' -- "$cur"); return ;;
        gui:--region|screen:--region)
            options='all'
            [[ $cmd == gui ]] && options+=' screen0'
            mapfile -t COMPREPLY < <(compgen -W "$options" -- "$cur"); return ;;
        config:-f|config:--filename|config:-m|config:--maincolor|config:-k|config:--contrastcolor|gui:-d|gui:--delay|screen:-d|screen:--delay|full:-d|full:--delay|screen:-n|screen:--number)
            return ;;
    esac
    case "$cmd" in
        gui) options='--path -p --clipboard -c --delay -d --region --last-region --raw -r --print-geometry -g --pin --accept-on-select -s' ;;
        screen) options='--number -n --edit -e --path -p --clipboard -c --delay -d --region --raw -r --pin' ;;
        full) options='--path -p --clipboard -c --delay -d --raw -r' ;;
        config) options='--autostart -a --filename -f --notifications -n --trayicon -t --showhelp -s --maincolor -m --contrastcolor -k --check' ;;
        help) mapfile -t COMPREPLY < <(compgen -W 'gui screen full config launcher' -- "$cur"); return ;;
        *) options='' ;;
    esac
    mapfile -t COMPREPLY < <(compgen -W "$options --help -h" -- "$cur")
}
complete -F _flameshot flameshot
