"""
Hardcoded ANSI ASCII art logos for ROS2 distros.
Fastfetch-quality, ~45 chars wide, 18-22 lines tall.
Works in ANY terminal — no dependencies required.
"""

# ── Color shortcuts ─────────────────────────────────────────────────────────
_RST = "\033[0m"
# Jazzy — cyan/green
_JC = "\033[38;2;0;210;235m"    # cyan
_JG = "\033[38;2;80;200;120m"   # green
_JY = "\033[38;2;240;200;50m"   # gold sombrero
_JD = "\033[38;2;0;150;170m"    # dark cyan
_JW = "\033[1;37m"              # white bold
# Humble — green/brown/gold
_HG = "\033[38;2;60;179;113m"   # sea green
_HB = "\033[38;2;139;90;43m"    # brown
_HD = "\033[38;2;34;120;80m"    # dark green
_HY = "\033[38;2;218;165;32m"   # gold
# Iron — steel blue/silver
_IB = "\033[38;2;70;130;180m"   # steel blue
_IS = "\033[38;2;192;192;192m"  # silver
_ID = "\033[38;2;45;85;130m"    # dark steel
_IG = "\033[38;2;140;160;180m"  # grey-blue
# Foxy — orange/red/white
_FO = "\033[38;2;255;140;0m"    # orange
_FR = "\033[38;2;220;50;32m"    # red
_FW = "\033[1;37m"              # white
_FY = "\033[38;2;255;180;50m"   # light orange
_FD = "\033[38;2;180;80;0m"     # dark orange
# Galactic — yellow/olive
_GY = "\033[38;2;230;200;50m"   # yellow
_GO = "\033[38;2;128;128;0m"    # olive
_GG = "\033[38;2;85;107;47m"    # dark olive
_GL = "\033[38;2;189;183;107m"  # khaki
# Eloquent — deep blue/grey
_EB = "\033[38;2;25;25;112m"    # midnight blue
_EG = "\033[38;2;140;140;160m"  # blue-grey
_EL = "\033[38;2;70;70;140m"    # med blue
_EW = "\033[38;2;200;200;220m"  # light
# Dashing — red/orange
_DR = "\033[38;2;220;40;30m"    # red
_DO = "\033[38;2;255;130;50m"   # orange
_DY = "\033[38;2;255;180;80m"   # yellow-orange
_DD = "\033[38;2;160;20;10m"    # dark red
# Rolling — white/grey
_RW = "\033[1;37m"              # white bold
_RG = "\033[38;2;150;150;150m"  # grey
_RL = "\033[38;2;100;100;100m"  # light grey
_RC = "\033[38;2;0;180;120m"    # ROS green


JAZZY = (
    f"{_JY}              ___________              {_RST}\n"
    f"{_JY}          .--'           '--.           {_RST}\n"
    f"{_JY}         /  ~ ~ ~ ~ ~ ~ ~ ~ \\          {_RST}\n"
    f"{_JY}        /____________________/\\         {_RST}\n"
    f"{_JY}        \\~~~~~~~~~~~~~~~~~~~~\\/         {_RST}\n"
    f"{_JC}            .'```````````'.              {_RST}\n"
    f"{_JC}          .'    _______    '.            {_RST}\n"
    f"{_JC}        .'   .-'       '-.   '.          {_RST}\n"
    f"{_JC}       /   .'    {_JW}0   0{_JC}    '.   \\         {_RST}\n"
    f"{_JC}      |   /                 \\   |        {_RST}\n"
    f"{_JC}      |  |    {_JG}\\         /{_JC}    |  |        {_RST}\n"
    f"{_JC}      |   \\    {_JG}'-------'{_JC}    /   |        {_RST}\n"
    f"{_JD}     /|    '.             .'    |\\       {_RST}\n"
    f"{_JD}    / |      '._________.'      | \\     {_RST}\n"
    f"{_JG}   /   \\    _.-|         |-._    /   \\   {_RST}\n"
    f"{_JG}  |  ___'--' / |         | \\ '--'___  |  {_RST}\n"
    f"{_JG}  |_/  \\____/  |_________|  \\____/  \\_|  {_RST}\n"
    f"{_JG}   \\__/                          \\__/    {_RST}\n"
    f"{_JD}        {_JW}Jazzy Jalisco{_JD} ~ ROS2           {_RST}\n"
)

HUMBLE = (
    f"{_HG}              .--~~--.                   {_RST}\n"
    f"{_HG}           .-'        '-.                {_RST}\n"
    f"{_HG}         .'    ______    '.              {_RST}\n"
    f"{_HG}        /    .'      '.    \\             {_RST}\n"
    f"{_HG}       /    /  {_HY}o    o{_HG}  \\    \\            {_RST}\n"
    f"{_HG}      |    |            |    |           {_RST}\n"
    f"{_HG}      |    |    {_HD}\\  /{_HG}     |    |           {_RST}\n"
    f"{_HG}      |     \\   {_HD}'--'{_HG}    /     |           {_RST}\n"
    f"{_HD}       \\     '._    _.'     /            {_RST}\n"
    f"{_HD}        \\       '~~'       /             {_RST}\n"
    f"{_HB}    _.---;\\    ________    /;---._        {_RST}\n"
    f"{_HB}   / _    \\'-./________\\.-'/    _ \\       {_RST}\n"
    f"{_HB}  / / '-._ \\  |      |  / _.-' \\ \\      {_RST}\n"
    f"{_HB}  | |     '-._|      |_.-'     | |      {_RST}\n"
    f"{_HB}  | |         |______|         | |      {_RST}\n"
    f"{_HB}  | '.___    / |    | \\    ___.' |      {_RST}\n"
    f"{_HB}   \\____ '--' /|    |\\ '--' ____/       {_RST}\n"
    f"{_HB}        '----' |    | '----'             {_RST}\n"
    f"{_HY}       Humble Hawksbill{_HB} ~ ROS2          {_RST}\n"
)

IRON = (
    f"{_IS}                   .                     {_RST}\n"
    f"{_IS}                  /|\\                    {_RST}\n"
    f"{_IS}                 / | \\                   {_RST}\n"
    f"{_IB}               .'  |  '.                 {_RST}\n"
    f"{_IB}             .'    |    '.               {_RST}\n"
    f"{_IB}           .'      |      '.             {_RST}\n"
    f"{_IB}         .'   .----+----.   '.           {_RST}\n"
    f"{_IB}       .'   .'    {_IG}o  o{_IB}    '.   '.         {_RST}\n"
    f"{_IB}     .'   .'                '.   '.       {_RST}\n"
    f"{_ID}   .'---.'    {_IS}~  ~  ~  ~{_ID}    '.---'.     {_RST}\n"
    f"{_ID}  /   .' '.                .' '.   \\    {_RST}\n"
    f"{_ID} /  .'     '-.          .-'     '.  \\   {_RST}\n"
    f"{_ID}/ .'          '-.  _.-'          '. \\  {_RST}\n"
    f"{_IB}|/               ''               \\|   {_RST}\n"
    f"{_IB}'               /  \\               '   {_RST}\n"
    f"{_IS}               /    \\                   {_RST}\n"
    f"{_IS}              /      \\                  {_RST}\n"
    f"{_IS}             '--------'                 {_RST}\n"
    f"{_IG}         Iron Irwini{_IS} ~ ROS2             {_RST}\n"
)

FOXY = (
    f"{_FO}           /\\      /\\                    {_RST}\n"
    f"{_FO}          /  \\    /  \\                   {_RST}\n"
    f"{_FO}         /    \\  /    \\                  {_RST}\n"
    f"{_FO}        /      \\/      \\                 {_RST}\n"
    f"{_FO}       /   {_FW}__        __{_FO}   \\                {_RST}\n"
    f"{_FO}      |   {_FW}(  )      (  ){_FO}   |               {_RST}\n"
    f"{_FO}      |   {_FW} \\/        \\/ {_FO}   |               {_RST}\n"
    f"{_FY}       \\        {_FW}/\\{_FY}        /                {_RST}\n"
    f"{_FY}        \\      {_FW}/  \\{_FY}      /                 {_RST}\n"
    f"{_FY}         \\    {_FW}/ .. \\{_FY}    /                  {_RST}\n"
    f"{_FO}          \\  {_FW}'------'{_FO}  /                   {_RST}\n"
    f"{_FO}           \\   {_FW}\\  /{_FO}   /                    {_RST}\n"
    f"{_FD}            \\   {_FW}\\/{_FD}   /                     {_RST}\n"
    f"{_FD}         .---'.    .'---.                 {_RST}\n"
    f"{_FD}        /      '~~'      \\                {_RST}\n"
    f"{_FD}       /                  \\               {_RST}\n"
    f"{_FO}      /    {_FW}~~~~~~~~~~~{_FO}     \\              {_RST}\n"
    f"{_FO}     '________________________'           {_RST}\n"
    f"{_FY}        Foxy Fitzroy{_FO} ~ ROS2             {_RST}\n"
)

GALACTIC = (
    f"{_GY}              .--------.                 {_RST}\n"
    f"{_GY}           .-'   ____   '-.              {_RST}\n"
    f"{_GY}         .'    .'    '.    '.            {_RST}\n"
    f"{_GO}        /     /  {_GY}o  o{_GO}  \\     \\           {_RST}\n"
    f"{_GO}       |     |          |     |          {_RST}\n"
    f"{_GO}       |     |   {_GG}\\  /{_GO}   |     |          {_RST}\n"
    f"{_GO}       |      \\  {_GG}'--'{_GO}  /      |          {_RST}\n"
    f"{_GO}        \\      '------'      /           {_RST}\n"
    f"{_GG}    _____\\__________________/_____       {_RST}\n"
    f"{_GG}   /    _.-'              '-._    \\      {_RST}\n"
    f"{_GG}  /   .'  |    |    |    |  '.   \\     {_RST}\n"
    f"{_GG}  |  /    |    |    |    |    \\  |     {_RST}\n"
    f"{_GG}  | |   __|____|____|____|__   | |     {_RST}\n"
    f"{_GG}  | |  /                    \\  | |     {_RST}\n"
    f"{_GO}  |  \\ \\   .--.    .--.   / /  |       {_RST}\n"
    f"{_GO}   \\  \\ '--'    '--'    '--' /  /       {_RST}\n"
    f"{_GO}    \\  '-----.      .-----'  /          {_RST}\n"
    f"{_GO}     '--------\\    /--------'            {_RST}\n"
    f"{_GL}     Galactic Geochelone{_GO} ~ ROS2         {_RST}\n"
)

ELOQUENT = (
    f"{_EG}                  _                      {_RST}\n"
    f"{_EG}                 / \\                     {_RST}\n"
    f"{_EG}                /   \\                    {_RST}\n"
    f"{_EL}              .'     '.                  {_RST}\n"
    f"{_EL}            .'  .---.  '.                {_RST}\n"
    f"{_EB}           /  .'     '.  \\               {_RST}\n"
    f"{_EB}          /  / {_EW}o     o{_EB} \\  \\              {_RST}\n"
    f"{_EB}         |  |           |  |             {_RST}\n"
    f"{_EB}         |  |    {_EG}===={_EB}   |  |             {_RST}\n"
    f"{_EB}          \\  \\   {_EG}\\  /{_EB}  /  /              {_RST}\n"
    f"{_EB}           \\  '. {_EG}'--'{_EB} .'  /               {_RST}\n"
    f"{_EL}            '.  '---'  .'                {_RST}\n"
    f"{_EL}          .--'\\       /'---.             {_RST}\n"
    f"{_EL}         / .---'-----'---. \\            {_RST}\n"
    f"{_EG}        / /    .-------.    \\ \\          {_RST}\n"
    f"{_EG}       | |   .'  ~ ~ ~  '.   | |        {_RST}\n"
    f"{_EG}       | |  /  ~ ~ ~ ~ ~  \\  | |        {_RST}\n"
    f"{_EG}        \\ \\_/               \\_/ /        {_RST}\n"
    f"{_EG}         '------.     .------'           {_RST}\n"
    f"{_EW}       Eloquent Elusor{_EG} ~ ROS2           {_RST}\n"
)

DASHING = (
    f"{_DR}              .-------.                  {_RST}\n"
    f"{_DR}           .-'    _    '-.               {_RST}\n"
    f"{_DR}          /   .--' '--.   \\              {_RST}\n"
    f"{_DR}         |  .'   {_DY}o o{_DR}   '.  |             {_RST}\n"
    f"{_DR}         |  |    {_DO}^^^{_DR}    |  |             {_RST}\n"
    f"{_DR}          \\  '.       .'  /              {_RST}\n"
    f"{_DR}           '-. '-----' .-'               {_RST}\n"
    f"{_DO}        _.-'   '-----'   '-._            {_RST}\n"
    f"{_DO}      .'  \\               /  '.          {_RST}\n"
    f"{_DO}    .'  ___\\     .---.   /___  '.        {_RST}\n"
    f"{_DO}   /  .'    \\   / ___ \\  /    '.  \\      {_RST}\n"
    f"{_DO}  / .'       \\ | |   | | /       '. \\    {_RST}\n"
    f"{_DD}  |/   .---.  \\|_|   |_|/  .---.   \\|   {_RST}\n"
    f"{_DD}  ||  /     \\  |       |  /     \\  ||   {_RST}\n"
    f"{_DD}  ||  |     |  |       |  |     |  ||   {_RST}\n"
    f"{_DD}  ||  \\     /  |       |  \\     /  ||   {_RST}\n"
    f"{_DD}  ||   '---'   |       |   '---'   ||   {_RST}\n"
    f"{_DD}   \\\\__________/       \\__________//    {_RST}\n"
    f"{_DY}       Dashing Diademata{_DR} ~ ROS2         {_RST}\n"
)

ROLLING = (
    f"{_RG}            .----------.                 {_RST}\n"
    f"{_RG}          .'     __     '.               {_RST}\n"
    f"{_RW}         /     .'  '.     \\              {_RST}\n"
    f"{_RW}        /     /  {_RC}ROS2{_RW}  \\     \\             {_RST}\n"
    f"{_RW}       |     |   {_RC}gear{_RW}   |     |            {_RST}\n"
    f"{_RW}       |     |        |     |            {_RST}\n"
    f"{_RW}        \\     \\      /     /             {_RST}\n"
    f"{_RW}     .---'\\    '----'    /'---.          {_RST}\n"
    f"{_RG}    /  .---+----+--+----+---.  \\         {_RST}\n"
    f"{_RG}   /  /    |  .'    '.  |    \\  \\        {_RST}\n"
    f"{_RG}  |  | .--.|.'        '.|.--. |  |       {_RST}\n"
    f"{_RG}  |--| |  ||    {_RC}.--. {_RG}   ||  | |--|       {_RST}\n"
    f"{_RG}  |  | '--'|'.  {_RC}'--'{_RG}  .'|'--' |  |       {_RST}\n"
    f"{_RG}   \\  \\    |  '.    .'  |    /  /        {_RST}\n"
    f"{_RG}    \\  '---+----'--'----+---'  /         {_RST}\n"
    f"{_RL}     '---./    '----'    \\.---'          {_RST}\n"
    f"{_RL}        /      /    \\      \\             {_RST}\n"
    f"{_RL}       '------'      '------'            {_RST}\n"
    f"{_RC}          Rolling{_RG} ~ ROS2                {_RST}\n"
)


LOGOS: dict[str, str] = {
    "jazzy":    JAZZY,
    "humble":   HUMBLE,
    "iron":     IRON,
    "foxy":     FOXY,
    "galactic": GALACTIC,
    "eloquent": ELOQUENT,
    "dashing":  DASHING,
    "rolling":  ROLLING,
}


def get_logo(distro: str) -> str:
    """Return ANSI logo for given ROS2 distro name. Falls back to JAZZY."""
    return LOGOS.get(distro.lower(), JAZZY)
