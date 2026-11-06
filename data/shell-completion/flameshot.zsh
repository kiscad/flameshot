#compdef flameshot
# ------------------------------------------------------------------------------
# Description
# -----------
#
#  Completion script for the flameshot command line interface
#  (https://github.com/flameshot-org/flameshot).
#
# ------------------------------------------------------------------------------
# How to use
# -------
#
# Copy this file to /usr/share/zsh/site-functions/_flameshot
#


# gui

_flameshot_gui_opts=(
    {-p,--path}'[Existing directory or new file to save to]':dir:_files
    {-c,--clipboard}'[Save the capture to the clipboard]'
    {-d,--delay}'[Delay time in milliseconds]:milliseconds:'
    "--region[Screenshot region to select]:region:(all screen0)"
    "--last-region[Repeat screenshot with previously selected region]"
    {-r,--raw}'[Print raw PNG capture]'
    {-g,--print-geometry}'[Print geometry of the selection in the format WxH+X+Y. Does nothing if raw is specified]'
    "--pin[Pin the capture to the screen]"
    {-s,--accept-on-select}'[Accept capture as soon as a selection is made]'
    {-h,--help}'[Show the available arguments]'
)

_flameshot_gui() {
    _arguments -s : \
    "$_flameshot_gui_opts[@]"
}


# screen

_flameshot_screen_opts=(
    {-n,--number}'[Define the screen to capture (starting from 0)]:number:'
    {-e,--edit}'[Interactively select and edit the screenshot region]'
    {-p,--path}'[Existing directory or new file to save to]':dir:_files
    {-c,--clipboard}'[Save the capture to the clipboard]'
    {-d,--delay}'[Delay time in milliseconds]:milliseconds:'
    "--region[Screenshot region to select]:region:(all)"
    {-r,--raw}'[Print raw PNG capture]'
    "--pin[Pin the capture to the screen]"
    {-h,--help}'[Show the available arguments]'
)

_flameshot_screen() {
    _arguments -s : \
    "$_flameshot_screen_opts[@]"
}


# full

_flameshot_full_opts=(
    {-p,--path}'[Existing directory or new file to save to]':dir:_files
    {-c,--clipboard}'[Save the capture to the clipboard]'
    {-d,--delay}'[Delay time in milliseconds]:milliseconds:'
    {-r,--raw}'[Print raw PNG capture]'
    {-h,--help}'[Show the available arguments]'
)

_flameshot_full() {
    _arguments -s : \
    "$_flameshot_full_opts[@]"
}


# config

_flameshot_config_opts=(
    {-a,--autostart}'[Enable or disable run at startup]:boolean:(true false)'
    {-f,--filename}'[Set the filename pattern]:pattern:'
    {-n,--notifications}'[Enable or disable notifications]:boolean:(true false)'
    {-t,--trayicon}'[Enable or disable the tray icon]:boolean:(true false)'
    {-s,--showhelp}'[Show capture help]:boolean:(true false)'
    {-m,--maincolor}'[Define the main UI color]:color:'
    {-k,--contrastcolor}'[Define the contrast UI color]:color:'
    "--check[Check the configuration for errors]"
    {-h,--help}'[Show the available arguments]'
)

_flameshot_config() {
    _arguments -s : \
    "$_flameshot_config_opts[@]"
}


# Main handle
_flameshot() {
    local curcontext="$curcontext" ret=1
    local state
    local -a line commands

    commands=(
        "gui:Start the manual capture in GUI mode"
        "screen:Capture a single screen (one monitor)"
        "full:Capture the entire desktop (all monitors)"
        "launcher:Open the capture launcher"
        "config:Configure Flameshot"
        "help:Show command help"
    )

    local help_text=$(command flameshot --help 2>/dev/null)
    [[ $help_text == *$'\n  history '* ]] && commands+=("history:Open upload history")
    [[ $help_text == *$'\n  updates '* ]] && commands+=("updates:Check for updates")
    _arguments -C -s -S -n \
        '(- 1 *)'{-v,-V,--version}"[display version information]" \
        '(- 1 *)'{-h,--help}'[display usage information]' \
        '1:cmd:->cmds' \
        '*:: :->args' && ret=0

    case "$state" in
        (cmds)
            _describe -t commands 'commands' commands
        ;;
        (args)
        local cmd
        cmd=$words[1]
        case "$cmd" in
            (gui)
                _flameshot_gui && ret=0
            ;;
            (screen)
                _flameshot_screen && ret=0
            ;;
            (full)
                _flameshot_full && ret=0
            ;;
            (config)
                _flameshot_config && ret=0
            ;;
            (launcher|history|updates)
                _arguments '(-h --help)'{-h,--help}'[Show help]' && ret=0
            ;;
            (*)
                _default && ret=0
            ;;
        esac
        ;;
        (*)
        ;;
    esac

    return ret
}

_flameshot

#
# Editor modelines  -  https://www.wireshark.org/tools/modelines.html
#
# Local variables:
# mode: sh
# c-basic-offset: 4
# tab-width: 4
# indent-tabs-mode: nil
# End:
#
# vi: set filetype=zsh shiftwidth=4 tabstop=4 expandtab:
# :indentSize=4:tabSize=4:noTabs=true:
#
