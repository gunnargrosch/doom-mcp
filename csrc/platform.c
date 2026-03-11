/*
 * platform.c - doomgeneric platform for doom-mcp
 *
 * No main() - Rust provides the entry point and calls doomgeneric_Create /
 * doomgeneric_Tick directly via FFI.  This file implements the required DG_
 * functions and exposes helpers for Rust to set key state and read game state.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#else
#include <sys/time.h>
#include <unistd.h>
#endif

#include "doomgeneric.h"
#include "doomkeys.h"
#include "doomtype.h"
#include "d_player.h"
#include "doomstat.h"
#include "m_fixed.h"
#include "i_system.h"
#include "p_mobj.h"
#include "p_local.h"
#include "r_main.h"
#include "p_tick.h"
#include "info.h"
#include "i_sound.h"
#include "d_ticcmd.h"

/* Fallback key definitions */
#ifndef KEY_FIRE
#define KEY_FIRE        0xa3
#endif
#ifndef KEY_USE
#define KEY_USE         0xa2
#endif
#ifndef KEY_STRAFELEFT
#define KEY_STRAFELEFT  0xa0
#endif
#ifndef KEY_STRAFERIGHT
#define KEY_STRAFERIGHT 0xa1
#endif

/* ---- Key State ---- */

static int current_keys[256];
static int reported_keys[256];

/* ---- Timing ---- */

static uint32_t start_ms = 0;
static uint32_t virtual_ms = 0;
static int use_virtual_time = 0;

static uint32_t get_time_ms(void)
{
#ifdef _WIN32
    return (uint32_t)GetTickCount();
#else
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return (uint32_t)(tv.tv_sec * 1000 + tv.tv_usec / 1000);
#endif
}

/* ---- DG Platform Functions ---- */

void DG_Init(void)
{
    start_ms = get_time_ms();
    memset(current_keys, 0, sizeof(current_keys));
    memset(reported_keys, 0, sizeof(reported_keys));
}

void DG_DrawFrame(void)
{
    /* No-op: Rust reads DG_ScreenBuffer directly after ticks. */
}

void DG_SleepMs(uint32_t ms)
{
    if (use_virtual_time) {
        /* Advance virtual clock so engine wait-loops make progress */
        virtual_ms += ms;
        return;
    }
    /* Real sleep during init */
    if (ms > 0) {
#ifdef _WIN32
        Sleep(1);
#else
        usleep(1000);
#endif
    }
}

uint32_t DG_GetTicksMs(void)
{
    if (use_virtual_time)
        return virtual_ms;
    return get_time_ms() - start_ms;
}

/* Start a new game on the given skill/episode/map.
 * Uses G_DeferedInitNew — the same path as the in-game "New Game" menu. */
void mcp_new_game(int skill, int episode, int map)
{
    extern void G_DeferedInitNew(int skill, int episode, int map);
    G_DeferedInitNew(skill, episode, map);
}

/* Switch to virtual time (call after doomgeneric_Create) */
void mcp_enable_virtual_time(void)
{
    virtual_ms = get_time_ms() - start_ms;
    use_virtual_time = 1;
}

/* Advance virtual clock by one game tic (1000/35 ≈ 28ms) */
void mcp_advance_tick(void)
{
    virtual_ms += 1000 / 35;
}

int DG_GetKey(int *pressed, unsigned char *doomKey)
{
    /* Diff current vs reported state to generate press/release events. */
    for (int i = 0; i < 256; i++) {
        if (current_keys[i] != reported_keys[i]) {
            *pressed = current_keys[i];
            *doomKey = (unsigned char)i;
            reported_keys[i] = current_keys[i];
            return 1;
        }
    }
    return 0;
}

void DG_SetWindowTitle(const char *title)
{
    (void)title;
}

/* ---- Helper Functions (called from Rust via FFI) ---- */

void mcp_set_key(unsigned char key, int pressed)
{
    current_keys[key] = pressed;
}

void mcp_clear_keys(void)
{
    memset(current_keys, 0, sizeof(current_keys));
}

/* Game state struct - layout must match the Rust #[repr(C)] definition. */
typedef struct {
    int health;
    int armor;
    int ammo_bullets;
    int ammo_shells;
    int ammo_cells;
    int ammo_rockets;
    int weapon;
    int kills;
    int items;
    int secrets;
    int x;
    int y;
    unsigned int angle;
    int episode;
    int map;
} mcp_game_state_t;

void mcp_get_game_state(mcp_game_state_t *state)
{
    player_t *p = &players[consoleplayer];

    state->health = p->health;
    state->armor  = p->armorpoints;
    state->ammo_bullets = p->ammo[0];
    state->ammo_shells  = p->ammo[1];
    state->ammo_cells   = p->ammo[2];
    state->ammo_rockets = p->ammo[3];
    state->weapon  = (int)p->readyweapon;
    state->kills   = p->killcount;
    state->items   = p->itemcount;
    state->secrets = p->secretcount;

    if (p->mo) {
        state->x     = p->mo->x >> FRACBITS;
        state->y     = p->mo->y >> FRACBITS;
        state->angle = (unsigned)((double)p->mo->angle * 360.0 / 4294967296.0);
    } else {
        state->x     = 0;
        state->y     = 0;
        state->angle = 0;
    }

    state->episode = gameepisode;
    state->map     = gamemap;
}

/* ---- Enemy Detection ---- */

typedef struct {
    int type;        /* mobjtype_t enum value */
    int health;
    int distance;    /* approximate distance in map units */
    int angle;       /* degrees relative to player facing: 0=ahead, +left, -right */
    int visible;     /* line of sight to player */
} mcp_enemy_info_t;

#define MAX_ENEMIES 16

int mcp_get_enemies(mcp_enemy_info_t *enemies, int max_count)
{
    extern thinker_t thinkercap;
    player_t *p = &players[consoleplayer];
    if (!p->mo) return 0;

    int count = 0;
    thinker_t *th;

    for (th = thinkercap.next; th != &thinkercap && count < max_count; th = th->next) {
        if (th->function.acp1 != (actionf_p1)P_MobjThinker)
            continue;

        mobj_t *mo = (mobj_t *)th;

        /* Only report shootable enemies that count as kills */
        if (!(mo->flags & MF_SHOOTABLE))
            continue;
        if (!(mo->flags & MF_COUNTKILL))
            continue;
        if (mo->health <= 0)
            continue;

        fixed_t dx = mo->x - p->mo->x;
        fixed_t dy = mo->y - p->mo->y;
        int dist = P_AproxDistance(dx, dy) >> FRACBITS;

        /* Only report enemies within ~1500 map units */
        if (dist > 1500)
            continue;

        /* Angle from player to enemy, relative to player's facing */
        angle_t abs_angle = R_PointToAngle2(p->mo->x, p->mo->y, mo->x, mo->y);
        int rel_angle = (int)((abs_angle - p->mo->angle) >> 24);
        /* Convert 0-255 to -180..+180 degrees */
        rel_angle = rel_angle * 360 / 256;
        if (rel_angle > 180) rel_angle -= 360;

        enemies[count].type     = (int)mo->type;
        enemies[count].health   = mo->health;
        enemies[count].distance = dist;
        enemies[count].angle    = rel_angle;
        enemies[count].visible  = P_CheckSight(p->mo, mo) ? 1 : 0;
        count++;
    }

    return count;
}

/* ---- Item Detection ---- */

typedef struct {
    int type;        /* mobjtype_t enum value */
    int distance;
    int angle;       /* relative to player facing */
} mcp_item_info_t;

int mcp_get_items(mcp_item_info_t *items, int max_count)
{
    extern thinker_t thinkercap;
    player_t *p = &players[consoleplayer];
    if (!p->mo) return 0;

    int count = 0;
    thinker_t *th;

    for (th = thinkercap.next; th != &thinkercap && count < max_count; th = th->next) {
        if (th->function.acp1 != (actionf_p1)P_MobjThinker)
            continue;

        mobj_t *mo = (mobj_t *)th;

        /* Only pickupable items */
        if (!(mo->flags & MF_SPECIAL))
            continue;

        fixed_t dx = mo->x - p->mo->x;
        fixed_t dy = mo->y - p->mo->y;
        int dist = P_AproxDistance(dx, dy) >> FRACBITS;

        /* Only report items within ~500 map units */
        if (dist > 500)
            continue;

        angle_t abs_angle = R_PointToAngle2(p->mo->x, p->mo->y, mo->x, mo->y);
        int rel_angle = (int)((abs_angle - p->mo->angle) >> 24);
        rel_angle = rel_angle * 360 / 256;
        if (rel_angle > 180) rel_angle -= 360;

        items[count].type     = (int)mo->type;
        items[count].distance = dist;
        items[count].angle    = rel_angle;
        count++;
    }

    return count;
}

/* ================================================================== */
/* Stubs for excluded SDL-dependent files                              */
/* ================================================================== */

/* --- i_system.c stubs --- */

void I_Init(void) {}
void I_BindVariables(void) {}

void I_Quit(void)
{
    exit(0);
}

void I_Error(char *error, ...)
{
    va_list argptr;
    va_start(argptr, error);
    vfprintf(stderr, error, argptr);
    va_end(argptr);
    fprintf(stderr, "\n");
    exit(1);
}

void I_Tactile(int on, int off, int total)
{
    (void)on; (void)off; (void)total;
}

boolean I_GetMemoryValue(unsigned int offset, void *value, int size)
{
    (void)offset; (void)value; (void)size;
    return false;
}

byte *I_ZoneBase(int *size)
{
    *size = 16 * 1024 * 1024; /* 16 MB */
    return (byte *)malloc((size_t)*size);
}

#define MAX_ATEXIT 32
static atexit_func_t atexit_funcs[MAX_ATEXIT];
static int num_atexit = 0;

void I_AtExit(atexit_func_t func, boolean run_if_error)
{
    (void)run_if_error;
    if (num_atexit < MAX_ATEXIT)
        atexit_funcs[num_atexit++] = func;
}

boolean I_ConsoleStdout(void) { return false; }
void I_WaitVBL(int count) { (void)count; }
void I_PrintBanner(char *text) { (void)text; }
void I_PrintDivider(void) {}
void I_PrintStartupBanner(char *desc) { (void)desc; }

/* --- i_timer.c stubs --- */

int I_GetTime(void)
{
    uint32_t ms = DG_GetTicksMs();
    return (ms * 35) / 1000;
}

int I_GetTimeMS(void)
{
    return (int)DG_GetTicksMs();
}

void I_Sleep(int ms)
{
    DG_SleepMs((uint32_t)ms);
}

void I_InitTimer(void) {}

/* --- i_sound.c stubs (no-op sound) --- */

static boolean NullInitSound(boolean _use_sfx_prefix) { (void)_use_sfx_prefix; return true; }
static void NullShutdownSound(void) {}
static int NullGetSfxLumpNum(sfxinfo_t *sfx) { (void)sfx; return 0; }
static void NullUpdateSound(void) {}
static void NullUpdateSoundParams(int ch, int vol, int sep) { (void)ch; (void)vol; (void)sep; }
static int NullStartSound(sfxinfo_t *sfx, int ch, int vol, int sep) { (void)sfx; (void)ch; (void)vol; (void)sep; return 0; }
static void NullStopSound(int ch) { (void)ch; }
static boolean NullSoundIsPlaying(int ch) { (void)ch; return false; }
static void NullCacheSounds(sfxinfo_t *s, int n) { (void)s; (void)n; }

sound_module_t DG_sound_module =
{
    NULL, 0,
    NullInitSound, NullShutdownSound, NullGetSfxLumpNum,
    NullUpdateSound, NullUpdateSoundParams, NullStartSound,
    NullStopSound, NullSoundIsPlaying, NullCacheSounds,
};

static boolean NullInitMusic(void) { return true; }
static void NullShutdownMusic(void) {}
static void NullSetMusicVolume(int vol) { (void)vol; }
static void NullPauseMusic(void) {}
static void NullResumeMusic(void) {}
static void *NullRegisterSong(void *data, int len) { (void)data; (void)len; return NULL; }
static void NullUnRegisterSong(void *h) { (void)h; }
static void NullPlaySong(void *h, boolean l) { (void)h; (void)l; }
static void NullStopSong(void) {}
static boolean NullMusicIsPlaying(void) { return false; }
static void NullPollMusic(void) {}

music_module_t DG_music_module =
{
    NULL, 0,
    NullInitMusic, NullShutdownMusic, NullSetMusicVolume,
    NullPauseMusic, NullResumeMusic,
    NullRegisterSong, NullUnRegisterSong, NullPlaySong,
    NullStopSong, NullMusicIsPlaying, NullPollMusic,
};

void I_InitSound(boolean use_sfx_prefix) { (void)use_sfx_prefix; }
void I_ShutdownSound(void) {}
int I_GetSfxLumpNum(sfxinfo_t *sfx) { (void)sfx; return 0; }
void I_UpdateSound(void) {}
void I_UpdateSoundParams(int ch, int vol, int sep) { (void)ch; (void)vol; (void)sep; }
int I_StartSound(sfxinfo_t *sfx, int ch, int vol, int sep) { (void)sfx; (void)ch; (void)vol; (void)sep; return 0; }
void I_StopSound(int ch) { (void)ch; }
boolean I_SoundIsPlaying(int ch) { (void)ch; return false; }
void I_PrecacheSounds(sfxinfo_t *s, int n) { (void)s; (void)n; }
void I_InitMusic(void) {}
void I_ShutdownMusic(void) {}
void I_SetMusicVolume(int vol) { (void)vol; }
void I_PauseSong(void) {}
void I_ResumeSong(void) {}
void *I_RegisterSong(void *data, int len) { (void)data; (void)len; return NULL; }
void I_UnRegisterSong(void *h) { (void)h; }
void I_PlaySong(void *h, boolean l) { (void)h; (void)l; }
void I_StopSong(void) {}
boolean I_MusicIsPlaying(void) { return false; }
void I_BindSoundVariables(void) {}
/* I_InitTimidityConfig is in dummy.c */

int snd_sfxdevice = 0;
int snd_musicdevice = 0;
int snd_samplerate = 0;
int snd_cachesize = 0;
int snd_maxslicetime_ms = 0;
char *snd_musiccmd = "";

/* --- i_joystick.c stubs --- */

void I_InitJoystick(void) {}
void I_ShutdownJoystick(void) {}
void I_UpdateJoystick(void) {}
void I_BindJoystickVariables(void) {}

/* --- i_cdmus.c stubs --- */

int cd_music_volume = 0;

/* i_input.c is compiled from doomgeneric source - provides I_GetEvent, I_InitInput, etc. */

/* --- w_main.c stubs (if not in engine) --- */

boolean W_ParseCommandLine(void) { return true; }

/* Networking stubs are provided by dummy.c and d_loop.c */
