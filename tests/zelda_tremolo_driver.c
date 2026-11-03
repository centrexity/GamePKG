/* Private ROM-in-RAM oracle; no game data or driver bytes are embedded here. */
#include "snes/spc.h"
#include <assert.h>
#include GAMEPKG_SPC_PLAYER_SOURCE

uint8_t apu_cpuRead(Apu *apu, uint16_t address) { return apu->ram[address]; }
void apu_cpuWrite(Apu *apu, uint16_t address, uint8_t value) {
  apu->ram[address] = value;
}

static void run_until(Spc *cpu, uint16_t start, uint16_t end) {
  cpu->pc = start;
  cpu->p = false;
  cpu->stopped = false;
  cpu->sp = 0xfd;
  unsigned steps = 0;
  while (cpu->pc != end && steps++ < 64) spc_runOpcode(cpu);
  assert(cpu->pc == end);
}

static unsigned sequencing(Apu *apu, Spc *cpu, SpcPlayer *player) {
  const unsigned depths[] = {0, 1, 127, 255};
  unsigned cases = 0;
  DspRegWriteHistory history = {0};
  player->reg_write_history = &history;
  for (unsigned channel = 0; channel < 8; channel++) {
    unsigned x = channel * 2;
    for (unsigned delay = 0; delay < 256; delay++) {
      for (unsigned held = 0; held < 256; held++) {
        for (unsigned d = 0; d < 4; d++) {
          unsigned depth = depths[d];
          for (unsigned fractional = 0; fractional < 2; fractional++) {
            Channel *c = &player->channel[channel];
            memset(c, 0, sizeof(*c));
            unsigned phase = (delay * 17 + held * 31) & 255;
            unsigned rate = (held * 13 + channel * 7 + 1) & 255;
            apu->ram[0x2e0 + x] = c->tremolo_delay_ticks = delay;
            apu->ram[0xc0 + x] = c->tremolo_hold_count = held;
            apu->ram[0xc1 + x] = c->tremolo_depth = depth;
            apu->ram[0x2d0 + x] = c->tremolo_count = phase;
            apu->ram[0x2d1 + x] = c->tremolo_rate = rate;
            apu->ram[0x321 + x] = c->final_volume = 73;
            apu->ram[0x210 + x] = c->channel_volume_master = 255;
            apu->ram[0x301 + x] = 255;
            c->channel_volume = 0xff00;
            apu->ram[0x59] = 255;
            player->master_volume = 0xff00;
            apu->ram[0x51] = player->main_tempo_accum = 193;
            apu->ram[0x47] = player->cur_chan_bit = 1u << channel;
            apu->ram[0x5e] = player->vol_dirty = 0;
            apu->ram[0x13] = player->did_affect_volumepitch_flag = 0;
            cpu->x = x;
            history.count = 0;
            if (fractional) {
              run_until(cpu, 0x10d5, 0x10e4);
              HandlePanAndSweep(player, c);
            } else {
              /* Skip the unrelated volume-fade prefix and stop before pan. */
              run_until(cpu, 0x0f4c, 0x0f78);
              Chan_HandleTick(player, c);
            }
            assert(c->final_volume == apu->ram[0x321 + x]);
            assert(c->tremolo_count == apu->ram[0x2d0 + x]);
            assert(c->tremolo_hold_count == apu->ram[0xc0 + x]);
            assert(c->tremolo_delay_ticks == delay);
            assert(c->tremolo_depth == depth && c->tremolo_rate == rate);
            assert(player->vol_dirty == apu->ram[0x5e]);
            assert(history.count == (depth && held == delay ? 2 : 0));
            cases++;
          }
        }
      }
    }
  }
  player->reg_write_history = NULL;
  return cases;
}

int main(void) {
  Apu *apu = calloc(1, sizeof(*apu));
  assert(apu);
  assert(fread(apu->ram, 1, sizeof(apu->ram), stdin) == sizeof(apu->ram));
  assert(fgetc(stdin) == EOF);
  Spc *cpu = spc_init(apu);
  assert(cpu);
  SpcPlayer *player = calloc(1, sizeof(*player));
  assert(player);
  unsigned cases = 0;
  for (unsigned channel = 0; channel < 8; channel++) {
    unsigned x = channel * 2;
    for (unsigned phase = 0; phase < 256; phase++) {
      for (unsigned depth = 0; depth < 256; depth++) {
        for (unsigned interpolate = 0; interpolate < 2; interpolate++) {
          unsigned rate = (phase * 37 + depth * 13 + channel) & 255;
          unsigned fraction = (depth * 53 + phase + channel) & 255;
          unsigned master = (phase * 17 + depth * 3 + channel) & 255;
          unsigned channel_master = (depth * 19 + channel * 31) & 255;
          unsigned volume = (phase * 23 + depth * 7 + channel) & 255;
          apu->ram[0x2d0 + x] = phase;
          apu->ram[0x2d1 + x] = rate;
          apu->ram[0xc1 + x] = depth;
          apu->ram[0x51] = fraction;
          apu->ram[0x59] = master;
          apu->ram[0x210 + x] = channel_master;
          apu->ram[0x301 + x] = volume;
          apu->ram[0x13] = 0;
          cpu->x = x;
          cpu->a = phase;
          cpu->p = false;
          cpu->stopped = false;
          cpu->sp = 0xfd;
          /* RET pops low then high; sentinel is outside the driver. */
          apu->ram[0x1fe] = 0;
          apu->ram[0x1ff] = 0x40;
          cpu->pc = interpolate ? 0x114e : 0x115b;
          unsigned steps = 0;
          while (cpu->pc != 0x4000 && steps++ < 64) spc_runOpcode(cpu);
          assert(cpu->pc == 0x4000);
          unsigned adjusted = (phase + (interpolate ? rate * fraction / 256 : 0)) & 255;
          unsigned triangle = (adjusted * 2) & 255;
          if (adjusted & 128) triangle ^= 255;
          unsigned scale = 255 - triangle * depth / 256;
          unsigned expected = master * scale / 256;
          expected = expected * channel_master / 256;
          expected = expected * volume / 256;
          expected = expected * expected / 256;
          assert(apu->ram[0x321 + x] == expected);
          Channel *c = &player->channel[channel];
          c->tremolo_count = phase;
          c->tremolo_depth = depth;
          c->tremolo_rate = rate;
          c->channel_volume_master = channel_master;
          c->channel_volume = volume << 8;
          player->master_volume = master << 8;
          player->main_tempo_accum = fraction;
          player->did_affect_volumepitch_flag = 0;
          if (interpolate) HandleTremolo(player, c);
          else CalcTremolo(player, c);
          assert(c->final_volume == expected);
          assert(c->tremolo_count == phase);
          assert(player->did_affect_volumepitch_flag == (interpolate ? 128 : 0));
          assert(apu->ram[0x13] == (interpolate ? 128 : 0));
          assert(apu->ram[0x2d0 + x] == phase);
          cases++;
        }
      }
    }
  }
  unsigned timing_cases = sequencing(apu, cpu, player);
  spc_free(cpu);
  free(player);
  free(apu);
  printf("Original SPC tremolo arithmetic: %u exact cases passed.\n", cases);
  printf("Original SPC tremolo tick/delay sequencing: %u exact cases passed.\n", timing_cases);
}
