<script setup>
/**
 * The walkthrough's controls, in the deck's footer while a replay runs.
 *
 * What a viewer reaches for (play, chapters) is a button; speed, captions,
 * the transcript, the bubble and leaving sit under one menu. Below them one
 * button that depends on the moment: back out of a question, or "try it
 * yourself", which simply hands the live demo to the viewer.
 */
import { computed, nextTick, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { DEFAULT_LOCALE, useViewerI18n } from '../i18n/index.js';
import { intlLocale } from '../data/format.js';
import { localeRouteName } from '../router.js';
import { backToMain, currentTrack, pause, replay, seek, setSpeed, stopReplay, togglePlay } from './replay.js';
import { chapterAt, formatTime, nextChapterStart, previousChapterStart } from './timeline.js';
import { openTranscript } from './chrome.js';

const { t, locale } = useViewerI18n();
const router = useRouter();

const SPEEDS = [1, 1.25, 1.5];
function speedText(s) {
  return `${new Intl.NumberFormat(intlLocale()).format(s)}×`;
}

const track = computed(() => currentTrack());
const chapters = computed(() => track.value?.chapters ?? []);
const chapterIndex = computed(() => (track.value ? chapterAt(track.value, replay.now) : -1));
const progress = computed(() => (track.value?.duration ? Math.min(100, (replay.now / track.value.duration) * 100) : 0));
const timeText = computed(() => `${formatTime(replay.now)} / ${formatTime(track.value?.duration ?? 0)}`);

function prevChapter() {
  seek(previousChapterStart(track.value, replay.now));
}
function nextChapter() {
  const to = nextChapterStart(track.value, replay.now);
  if (to != null) seek(to);
}
function tryIt() {
  // The demo is live; pausing is all it takes. Play brings the recorded
  // moment back.
  pause();
  replay.diverged = true;
  replay.cursor.visible = false;
}
async function leave() {
  // Away from the walkthrough's page first: stopping remounts the tabs, and
  // on that page a fresh mount would start the walkthrough again.
  await router.push(router.resolve({ name: localeRouteName('home', locale.value) }).path);
  stopReplay();
}

const backButton = ref(null);
const playButton = ref(null);
// When a question opens, the question that had focus is gone from the rail;
// hand focus to the way back, and to play on return.
watch(
  () => replay.faq,
  (f, old) => nextTick(() => (f ? backButton.value : old ? playButton.value : null)?.focus?.()),
);
</script>

<template>
  <div class="controls wt-chrome" :lang="locale">
    <!-- The recording, its slides and the demo under it are Dutch in every
         interface language; the controls are the viewer's. -->
    <nldd-text v-if="locale !== DEFAULT_LOCALE" size="sm" color="inherit">{{ t('walkthrough.dutch_only') }}</nldd-text>
    <!-- Generated speech that passes for a person is labelled as such (AI
         Act, art. 50). Always in view, not only on the first slide: a viewer
         can start at any chapter. -->
    <nldd-text v-if="track?.generatedVoice" size="sm" color="inherit">{{ t('walkthrough.ai_voice') }}</nldd-text>
    <nldd-progress-bar size="sm" color="donkergeel" :value="progress" max="100" value-display="none" :accessible-label="t('walkthrough.progress_at', { time: timeText })"></nldd-progress-bar>
    <div class="control-row">
      <nldd-button-bar>
        <nldd-icon-button variant="inherit-tinted" icon="media-backward-end" :text="t('walkthrough.prev_chapter')" @click="prevChapter"></nldd-icon-button>
        <nldd-icon-button ref="playButton" variant="inherit-filled" :icon="replay.playing ? 'pause' : 'play'" :text="replay.playing ? t('walkthrough.pause') : t('walkthrough.play')" @click="togglePlay"></nldd-icon-button>
        <nldd-icon-button variant="inherit-tinted" icon="media-forward-end" :text="t('walkthrough.next_chapter')" @click="nextChapter"></nldd-icon-button>
      </nldd-button-bar>
      <span class="time" aria-hidden="true">{{ timeText }}</span>
      <nldd-button-bar>
        <nldd-button variant="inherit-tinted" size="sm" :text="t('walkthrough.chapters')" expandable popup-type="menu">
          <nldd-menu slot="popup" placement="top-start" :accessible-label="t('walkthrough.chapters')">
            <nldd-menu-item
              v-for="(c, i) in chapters"
              :key="i"
              type="radio"
              :text="c.slide?.title ?? c.slide?.lines?.[0]?.replaceAll('**', '') ?? t('walkthrough.chapter', { n: i + 1 })"
              :details="formatTime(c.start)"
              :selected="i === chapterIndex || undefined"
              @select="seek(c.start)"
            ></nldd-menu-item>
          </nldd-menu>
        </nldd-button>
        <nldd-icon-button variant="inherit-tinted" icon="more" :text="t('walkthrough.more')" expandable popup-type="menu">
          <nldd-menu slot="popup" placement="top-end" :accessible-label="t('walkthrough.more')">
            <!-- `@select` on each item, not on the group: the same reason as
                 the toolbar menus in App.vue. -->
            <nldd-menu-group :text="t('walkthrough.speed')">
              <nldd-menu-item v-for="s in SPEEDS" :key="s" type="radio" :text="speedText(s)" :selected="s === replay.speed || undefined" @select="setSpeed(s)"></nldd-menu-item>
            </nldd-menu-group>
            <nldd-menu-group :text="t('walkthrough.view')">
              <nldd-menu-item type="checkbox" icon="message-rectangle-text" :text="t('walkthrough.captions')" :selected="replay.captionsOn || undefined" @select="replay.captionsOn = !replay.captionsOn"></nldd-menu-item>
              <nldd-menu-item v-if="track?.cam" type="checkbox" icon="person-circle" :text="t('walkthrough.camera')" :selected="replay.camOn || undefined" @select="replay.camOn = !replay.camOn"></nldd-menu-item>
              <nldd-menu-item icon="text-document" :text="t('walkthrough.transcript.open')" @select="openTranscript"></nldd-menu-item>
            </nldd-menu-group>
            <nldd-menu-group>
              <nldd-menu-item icon="home" :text="t('walkthrough.leave')" @select="leave"></nldd-menu-item>
            </nldd-menu-group>
          </nldd-menu>
        </nldd-icon-button>
      </nldd-button-bar>
    </div>
    <nldd-button v-if="replay.faq" ref="backButton" variant="inherit-filled" size="sm" start-icon="back" :text="t('walkthrough.faq.back')" @click="backToMain"></nldd-button>
    <nldd-text v-else-if="replay.diverged" size="sm" color="inherit">{{ t('walkthrough.own_turn') }}</nldd-text>
    <nldd-button v-else variant="inherit-tinted" size="sm" start-icon="hand" :text="t('walkthrough.try')" @click="tryIt"></nldd-button>
  </div>
</template>

<style scoped>
/* Layout only; the controls are nldd components. Custom CSS: the deck has no
   player footer of its own. */
.controls {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  padding-top: 1rem;
}
.controls > nldd-button {
  align-self: flex-start;
}
.control-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem 1rem;
}
.time {
  font-variant-numeric: tabular-nums;
  font-size: 0.9rem;
  opacity: 0.8;
}
</style>
