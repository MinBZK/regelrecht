<script setup lang="ts">
import { computed } from 'vue'
import { useData, useRoute } from 'vitepress'
import { content, langFromPath } from './content'

const route = useRoute()
const { frontmatter } = useData()

const lang = computed(() => langFromPath(route.path))
const t = computed(() => content[lang.value])
const home = computed(() => (lang.value === 'en' ? '/en/' : '/'))
const showSignup = computed(() => frontmatter.value.page === 'aanmelden')
</script>

<template>
  <!-- Wrapper, skip-link, nav and footer live in the shared Layout.
       This renders only the landing's page content.

       NLDD-component strategy (keeps the no-JS / SSR a11y intact):
       - nldd-title / nldd-container / nldd-tag / nldd-card render their
         content via a light-DOM <slot>, so the real <h1>/<h2>/<p>/<a>
         is server-rendered and works without JS; only the visual chrome
         needs the upgraded component. (nldd-box is intentionally NOT
         used for the card grids: it has no grid layout and would stack
         the cards flush — a plain CSS grid wraps them instead, inside
         the section's nldd-container.)
       - nldd-rich-text uses NO shadow DOM at all (light DOM) — fully
         SSR-safe.
       - nldd-button is the exception: its text is a `text=` attribute
         rendered in shadow DOM, invisible without JS. So every button
         ships a real <a class="rr-btn rr-btn-fallback"> next to an
         <nldd-button class="rr-btn-wc">; CSS shows exactly one
         (fallback until the component is :defined, then the component). -->
  <div>
    <!-- ===================== SIGNUP PAGE ===================== -->
    <template v-if="showSignup">
      <section class="rr-section" aria-labelledby="rr-signup-title">
        <nldd-container padding="0" class="rr-container">
          <nldd-title size="1">
            <h1 id="rr-signup-title" class="rr-section-title">
              {{ t.signup.pageTitle }}
            </h1>
          </nldd-title>
          <nldd-rich-text>
            <p class="rr-lede">{{ t.signup.lede }}</p>
          </nldd-rich-text>
          <ClientOnly>
            <SignupForm />
          </ClientOnly>
          <noscript>
            <p>{{ t.signup.noscript }}</p>
          </noscript>
        </nldd-container>
      </section>
    </template>

    <!-- ===================== LANDING ===================== -->
    <template v-else>
      <section class="rr-hero" aria-labelledby="rr-hero-title">
        <nldd-container padding="0" class="rr-container">
          <nldd-title size="1">
            <h1 id="rr-hero-title">
              RegelRecht
              <small>{{ t.hero.titleSmall }}</small>
            </h1>
          </nldd-title>
          <nldd-rich-text>
            <p>{{ t.hero.intro }}</p>
          </nldd-rich-text>
          <p class="rr-cta">
            <nldd-button
              class="rr-btn-wc"
              variant="primary"
              :href="home + '#what-is-it'"
              :text="t.hero.cta"
            />
            <a
              class="rr-btn rr-btn--primary rr-btn-fallback"
              :href="home + '#what-is-it'"
              >{{ t.hero.cta }}</a
            >
          </p>
        </nldd-container>
      </section>

      <section
        class="rr-section rr-partners"
        aria-labelledby="rr-partners-title"
      >
        <nldd-container padding="0" class="rr-container">
          <h2 id="rr-partners-title" class="rr-visually-hidden">
            {{ t.partners.label }}
          </h2>
          <p>{{ t.partners.label }}</p>
          <ul>
            <li v-for="p in t.partners.items" :key="p.href">
              <a :href="p.href">{{ p.label }}</a>
            </li>
          </ul>
        </nldd-container>
      </section>

      <section
        id="what-is-it"
        class="rr-section rr-section--alt"
        aria-labelledby="rr-what-title"
      >
        <nldd-container padding="0" class="rr-container">
          <nldd-title size="2">
            <h2 id="rr-what-title" class="rr-section-title">
              {{ t.whatIsIt.title }}
            </h2>
          </nldd-title>
          <nldd-rich-text>
            <p class="rr-lede">{{ t.whatIsIt.lede }}</p>
          </nldd-rich-text>
          <div class="rr-grid">
            <nldd-card
              v-for="c in t.whatIsIt.cards"
              :key="c.h"
              class="rr-card"
              :accessible-label="c.h"
            >
              <nldd-rich-text class="rr-card-inner">
                <h3>{{ c.h }}</h3>
                <p>{{ c.p }}</p>
              </nldd-rich-text>
            </nldd-card>
          </div>
        </nldd-container>
      </section>

      <section
        id="why-important"
        class="rr-section"
        aria-labelledby="rr-why-title"
      >
        <nldd-container padding="0" class="rr-container">
          <nldd-title size="2">
            <h2 id="rr-why-title" class="rr-section-title">
              {{ t.whyImportant.title }}
            </h2>
          </nldd-title>
          <nldd-rich-text>
            <p class="rr-lede">{{ t.whyImportant.lede }}</p>
          </nldd-rich-text>
          <div class="rr-ps-list">
            <div
              v-for="ps in t.whyImportant.problemSolutions"
              :key="ps.problemTitle"
              class="rr-ps"
              role="group"
              :aria-label="ps.problemTitle + ' — ' + ps.solutionTitle"
            >
              <div class="rr-ps-problem">
                <nldd-tag color="critical" size="md">{{
                  lang === 'en' ? 'The problem' : 'Het probleem'
                }}</nldd-tag>
                <h3>{{ ps.problemTitle }}</h3>
                <p>{{ ps.problemText }}</p>
              </div>
              <div class="rr-ps-solution">
                <nldd-tag color="accent" size="md">{{
                  lang === 'en' ? 'Possible direction' : 'Mogelijke richting'
                }}</nldd-tag>
                <h3>{{ ps.solutionTitle }}</h3>
                <p>{{ ps.solutionText }}</p>
              </div>
            </div>
          </div>
        </nldd-container>
      </section>

      <section
        id="how-it-works"
        class="rr-section rr-section--alt"
        aria-labelledby="rr-how-title"
      >
        <nldd-container padding="0" class="rr-container">
          <nldd-title size="2">
            <h2 id="rr-how-title" class="rr-section-title">
              {{ t.howItWorks.title }}
            </h2>
          </nldd-title>
          <nldd-rich-text>
            <p class="rr-lede">{{ t.howItWorks.lede }}</p>
          </nldd-rich-text>
          <ol class="rr-steps">
            <li v-for="step in t.howItWorks.steps" :key="step.title">
              <h3>{{ step.title }}</h3>
              <p>{{ step.text }}</p>
            </li>
          </ol>
        </nldd-container>
      </section>

      <section id="tools" class="rr-section" aria-labelledby="rr-tools-title">
        <nldd-container padding="0" class="rr-container">
          <nldd-title size="2">
            <h2 id="rr-tools-title" class="rr-section-title">
              {{ t.tools.title }}
            </h2>
          </nldd-title>
          <div class="rr-grid">
            <nldd-card
              v-for="tool in t.tools.items"
              :key="tool.title"
              class="rr-card"
              :accessible-label="tool.title"
            >
              <div class="rr-card-inner">
                <a
                  v-if="tool.link"
                  class="rr-card-link"
                  :href="tool.link.href"
                  >{{ tool.link.label }}</a
                >
                <nldd-tag v-else color="neutral" size="md">{{
                  tool.meta
                }}</nldd-tag>
                <nldd-rich-text>
                  <h3>{{ tool.title }}</h3>
                  <p>{{ tool.text }}</p>
                </nldd-rich-text>
              </div>
            </nldd-card>
          </div>
        </nldd-container>
      </section>

      <section
        id="example"
        class="rr-section rr-section--alt"
        aria-labelledby="rr-example-title"
      >
        <nldd-container padding="0" class="rr-container">
          <nldd-title size="2">
            <h2 id="rr-example-title" class="rr-section-title">
              {{ t.example.title }}
            </h2>
          </nldd-title>
          <nldd-rich-text>
            <p class="rr-lede">{{ t.example.lede }}</p>
          </nldd-rich-text>

          <div
            v-for="ex in t.example.cases"
            :key="ex.h"
            class="rr-example"
            :class="{ 'rr-example--reverse': ex.reverse }"
          >
            <figure>
              <img :src="ex.img" :alt="ex.alt" />
              <figcaption>{{ ex.caption }}</figcaption>
            </figure>
            <nldd-rich-text>
              <h3>{{ ex.h }}</h3>
              <p>{{ ex.p }}</p>
              <ul>
                <li v-for="b in ex.bullets" :key="b">{{ b }}</li>
              </ul>
            </nldd-rich-text>
          </div>
        </nldd-container>
      </section>

      <section
        id="innovation"
        class="rr-section"
        aria-labelledby="rr-innovation-title"
      >
        <nldd-container padding="0" class="rr-container">
          <nldd-title size="2">
            <h2 id="rr-innovation-title" class="rr-section-title">
              {{ t.innovation.title }}
            </h2>
          </nldd-title>
          <nldd-rich-text>
            <p class="rr-lede">
              {{ t.innovation.ledeBefore
              }}<a :href="t.innovation.ledeLink.href">{{
                t.innovation.ledeLink.label
              }}</a
              >{{ t.innovation.ledeAfter }}
            </p>
          </nldd-rich-text>
          <div class="rr-grid">
            <nldd-card
              v-for="c in t.innovation.cards"
              :key="c.h"
              class="rr-card"
              :accessible-label="c.h"
            >
              <div class="rr-card-inner">
                <nldd-tag color="accent" size="md">{{ c.meta }}</nldd-tag>
                <nldd-rich-text>
                  <h3>{{ c.h }}</h3>
                  <p>{{ c.p }}</p>
                </nldd-rich-text>
              </div>
            </nldd-card>
          </div>
        </nldd-container>
      </section>

      <section
        id="references"
        class="rr-section rr-section--alt"
        aria-labelledby="rr-refs-title"
      >
        <nldd-container padding="0" class="rr-container">
          <nldd-title size="2">
            <h2 id="rr-refs-title" class="rr-section-title">
              {{ t.references.title }}
            </h2>
          </nldd-title>
          <nldd-rich-text>
            <p class="rr-lede">{{ t.references.lede }}</p>
          </nldd-rich-text>
          <ul class="rr-refs">
            <li v-for="ref in t.references.items" :key="ref.title">
              <nldd-rich-text>
                <h3>{{ ref.title }}</h3>
                <p class="rr-ref-meta">{{ ref.meta }}</p>
                <p>{{ ref.text }}</p>
              </nldd-rich-text>
              <a :href="ref.href">
                {{ ref.linkLabel }}
                <span class="rr-visually-hidden">— {{ ref.title }}</span>
              </a>
            </li>
          </ul>
        </nldd-container>
      </section>

      <section id="faq" class="rr-section" aria-labelledby="rr-faq-title">
        <nldd-container padding="0" class="rr-container">
          <nldd-title size="2">
            <h2 id="rr-faq-title" class="rr-section-title">
              {{ t.faq.title }}
            </h2>
          </nldd-title>
          <div class="rr-faq">
            <details v-for="item in t.faq.items" :key="item.q">
              <summary>{{ item.q }}</summary>
              <nldd-rich-text>
                <p>{{ item.a }}</p>
              </nldd-rich-text>
            </details>
          </div>
        </nldd-container>
      </section>

      <section
        id="feedback"
        class="rr-section rr-section--alt rr-feedback"
        aria-labelledby="rr-feedback-title"
      >
        <nldd-container padding="0" class="rr-container">
          <nldd-title size="2">
            <h2 id="rr-feedback-title" class="rr-section-title">
              {{ t.feedback.title }}
            </h2>
          </nldd-title>
          <nldd-rich-text>
            <p>{{ t.feedback.body }}</p>
          </nldd-rich-text>
          <p class="rr-cta">
            <nldd-button
              class="rr-btn-wc"
              variant="primary"
              :href="t.feedback.ctaHref"
              :text="t.feedback.cta"
            />
            <a
              class="rr-btn rr-btn--primary rr-btn-fallback"
              :href="t.feedback.ctaHref"
              >{{ t.feedback.cta }}</a
            >
          </p>
        </nldd-container>
      </section>
    </template>
  </div>
</template>
