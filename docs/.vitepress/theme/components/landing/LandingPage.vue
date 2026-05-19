<script setup lang="ts">
import { computed } from 'vue'
import { useData, useRoute } from 'vitepress'
import { content, langFromPath } from './content'
import RrFooter from './RrFooter.vue'

const route = useRoute()
const { frontmatter } = useData()

const lang = computed(() => langFromPath(route.path))
const t = computed(() => content[lang.value])
const home = computed(() => (lang.value === 'en' ? '/en/' : '/'))
const showSignup = computed(() => frontmatter.value.page === 'aanmelden')
</script>

<template>
  <div class="rr-landing" :lang="lang">
    <a class="rr-skip-link" href="#rr-main">
      {{ lang === 'en' ? 'Skip to content' : 'Direct naar de inhoud' }}
    </a>

    <RrNav />

    <main id="rr-main">
      <!-- ===================== SIGNUP PAGE ===================== -->
      <template v-if="showSignup">
        <section class="rr-section" aria-labelledby="rr-signup-title">
          <div class="rr-container">
            <h1 id="rr-signup-title" class="rr-section-title">
              {{ t.signup.pageTitle }}
            </h1>
            <p class="rr-lede">{{ t.signup.lede }}</p>
            <ClientOnly>
              <SignupForm />
            </ClientOnly>
            <noscript>
              <p>{{ t.signup.noscript }}</p>
            </noscript>
          </div>
        </section>
      </template>

      <!-- ===================== LANDING ===================== -->
      <template v-else>
        <section class="rr-hero" aria-labelledby="rr-hero-title">
          <div class="rr-container">
            <h1 id="rr-hero-title">
              RegelRecht
              <small>{{ t.hero.titleSmall }}</small>
            </h1>
            <p>{{ t.hero.intro }}</p>
            <p>
              <a class="rr-btn rr-btn--primary" :href="home + '#what-is-it'">
                {{ t.hero.cta }}
              </a>
            </p>
          </div>
        </section>

        <section
          class="rr-section rr-partners"
          aria-labelledby="rr-partners-title"
        >
          <div class="rr-container">
            <h2 id="rr-partners-title" class="rr-visually-hidden">
              {{ t.partners.label }}
            </h2>
            <p>{{ t.partners.label }}</p>
            <ul>
              <li v-for="p in t.partners.items" :key="p.href">
                <a :href="p.href">{{ p.label }}</a>
              </li>
            </ul>
          </div>
        </section>

        <section
          id="what-is-it"
          class="rr-section rr-section--alt"
          aria-labelledby="rr-what-title"
        >
          <div class="rr-container">
            <h2 id="rr-what-title" class="rr-section-title">
              {{ t.whatIsIt.title }}
            </h2>
            <p class="rr-lede">{{ t.whatIsIt.lede }}</p>
            <div class="rr-grid">
              <nldd-card
                v-for="c in t.whatIsIt.cards"
                :key="c.h"
                class="rr-card"
                :accessible-label="c.h"
              >
                <div class="rr-card-inner">
                  <h3>{{ c.h }}</h3>
                  <p>{{ c.p }}</p>
                </div>
              </nldd-card>
            </div>
          </div>
        </section>

        <section
          id="why-important"
          class="rr-section"
          aria-labelledby="rr-why-title"
        >
          <div class="rr-container">
            <h2 id="rr-why-title" class="rr-section-title">
              {{ t.whyImportant.title }}
            </h2>
            <p class="rr-lede">{{ t.whyImportant.lede }}</p>
            <div class="rr-ps-list">
              <div
                v-for="ps in t.whyImportant.problemSolutions"
                :key="ps.problemTitle"
                class="rr-ps"
                role="group"
                :aria-label="ps.problemTitle + ' — ' + ps.solutionTitle"
              >
                <div class="rr-ps-problem">
                  <span class="rr-ps-tag">{{
                    lang === 'en' ? 'The problem' : 'Het probleem'
                  }}</span>
                  <h3>{{ ps.problemTitle }}</h3>
                  <p>{{ ps.problemText }}</p>
                </div>
                <div class="rr-ps-solution">
                  <span class="rr-ps-tag">{{
                    lang === 'en' ? 'Possible direction' : 'Mogelijke richting'
                  }}</span>
                  <h3>{{ ps.solutionTitle }}</h3>
                  <p>{{ ps.solutionText }}</p>
                </div>
              </div>
            </div>
          </div>
        </section>

        <section
          id="how-it-works"
          class="rr-section rr-section--alt"
          aria-labelledby="rr-how-title"
        >
          <div class="rr-container">
            <h2 id="rr-how-title" class="rr-section-title">
              {{ t.howItWorks.title }}
            </h2>
            <p class="rr-lede">{{ t.howItWorks.lede }}</p>
            <ol class="rr-steps">
              <li v-for="step in t.howItWorks.steps" :key="step.title">
                <h3>{{ step.title }}</h3>
                <p>{{ step.text }}</p>
              </li>
            </ol>
          </div>
        </section>

        <section id="tools" class="rr-section" aria-labelledby="rr-tools-title">
          <div class="rr-container">
            <h2 id="rr-tools-title" class="rr-section-title">
              {{ t.tools.title }}
            </h2>
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
                  <span v-else class="rr-card-meta">{{ tool.meta }}</span>
                  <h3>{{ tool.title }}</h3>
                  <p>{{ tool.text }}</p>
                </div>
              </nldd-card>
            </div>
          </div>
        </section>

        <section
          id="example"
          class="rr-section rr-section--alt"
          aria-labelledby="rr-example-title"
        >
          <div class="rr-container">
            <h2 id="rr-example-title" class="rr-section-title">
              {{ t.example.title }}
            </h2>
            <p class="rr-lede">{{ t.example.lede }}</p>

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
              <div>
                <h3>{{ ex.h }}</h3>
                <p>{{ ex.p }}</p>
                <ul>
                  <li v-for="b in ex.bullets" :key="b">{{ b }}</li>
                </ul>
              </div>
            </div>
          </div>
        </section>

        <section
          id="innovation"
          class="rr-section"
          aria-labelledby="rr-innovation-title"
        >
          <div class="rr-container">
            <h2 id="rr-innovation-title" class="rr-section-title">
              {{ t.innovation.title }}
            </h2>
            <p class="rr-lede">
              {{ t.innovation.ledeBefore
              }}<a :href="t.innovation.ledeLink.href">{{
                t.innovation.ledeLink.label
              }}</a
              >{{ t.innovation.ledeAfter }}
            </p>
            <div class="rr-grid">
              <nldd-card
                v-for="c in t.innovation.cards"
                :key="c.h"
                class="rr-card"
                :accessible-label="c.h"
              >
                <div class="rr-card-inner">
                  <span class="rr-card-meta">{{ c.meta }}</span>
                  <h3>{{ c.h }}</h3>
                  <p>{{ c.p }}</p>
                </div>
              </nldd-card>
            </div>
          </div>
        </section>

        <section
          id="references"
          class="rr-section rr-section--alt"
          aria-labelledby="rr-refs-title"
        >
          <div class="rr-container">
            <h2 id="rr-refs-title" class="rr-section-title">
              {{ t.references.title }}
            </h2>
            <p class="rr-lede">{{ t.references.lede }}</p>
            <ul class="rr-refs">
              <li v-for="ref in t.references.items" :key="ref.title">
                <h3>{{ ref.title }}</h3>
                <p class="rr-ref-meta">{{ ref.meta }}</p>
                <p>{{ ref.text }}</p>
                <a :href="ref.href">
                  {{ ref.linkLabel }}
                  <span class="rr-visually-hidden">— {{ ref.title }}</span>
                </a>
              </li>
            </ul>
          </div>
        </section>

        <section id="faq" class="rr-section" aria-labelledby="rr-faq-title">
          <div class="rr-container">
            <h2 id="rr-faq-title" class="rr-section-title">
              {{ t.faq.title }}
            </h2>
            <div class="rr-faq">
              <details v-for="item in t.faq.items" :key="item.q">
                <summary>{{ item.q }}</summary>
                <p>{{ item.a }}</p>
              </details>
            </div>
          </div>
        </section>

        <section
          id="feedback"
          class="rr-section rr-section--alt rr-feedback"
          aria-labelledby="rr-feedback-title"
        >
          <div class="rr-container">
            <h2 id="rr-feedback-title" class="rr-section-title">
              {{ t.feedback.title }}
            </h2>
            <p>{{ t.feedback.body }}</p>
            <p>
              <a class="rr-btn rr-btn--primary" :href="t.feedback.ctaHref">
                {{ t.feedback.cta }}
              </a>
            </p>
          </div>
        </section>
      </template>
    </main>

    <RrFooter />
  </div>
</template>
