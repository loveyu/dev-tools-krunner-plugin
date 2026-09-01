<script setup lang="ts">
import { computed, ref } from 'vue';
import { NAlert, NButton, NCard, NInput, NSelect, NSpace, NTag, useMessage } from 'naive-ui';

import { useI18n } from '../i18n/runtime';
import { postRequest } from '../ipc/bridge';
import {
  CIPHER_ALGORITHMS,
  decryptText,
  encryptText,
  isLegacyCipher,
  IT_TOOLS_PROJECT_URL,
  type CipherAlgorithm,
} from '../tools/crypto/model';

defineOptions({ name: 'CryptoView' });

const { t } = useI18n();
const message = useMessage();
const encryptAlgorithm = ref<CipherAlgorithm>('AES');
const decryptAlgorithm = ref<CipherAlgorithm>('AES');
const encryptInput = ref('');
const encryptPassphrase = ref('');
const encryptOutput = ref('');
const decryptInput = ref('');
const decryptPassphrase = ref('');
const decryptOutput = ref('');
const encryptError = ref<string | null>(null);
const decryptError = ref<string | null>(null);
const algorithmOptions = computed(() =>
  CIPHER_ALGORITHMS.map((algorithm) => ({ label: algorithm, value: algorithm })),
);

function runEncrypt(): void {
  encryptError.value = null;
  try {
    encryptOutput.value = encryptText(
      encryptAlgorithm.value,
      encryptInput.value,
      encryptPassphrase.value,
    );
  } catch (error) {
    encryptError.value = error instanceof Error ? error.message : String(error);
  }
}

function runDecrypt(): void {
  decryptError.value = null;
  try {
    decryptOutput.value = decryptText(
      decryptAlgorithm.value,
      decryptInput.value,
      decryptPassphrase.value,
    );
  } catch (error) {
    decryptError.value = error instanceof Error ? error.message : String(error);
  }
}

function copy(value: string): void {
  if (value !== '' && postRequest({ type: 'clipboardWrite', text: value })) {
    message.success(t('crypto.messages.copied'));
  }
}

function moveToDecrypt(): void {
  decryptInput.value = encryptOutput.value;
  decryptAlgorithm.value = encryptAlgorithm.value;
}
</script>

<template>
  <main class="crypto-view">
    <header>
      <div>
        <h1>{{ t('crypto.title') }}</h1>
        <p>{{ t('crypto.description') }}</p>
      </div>
      <NTag :bordered="false" type="info">{{ t('crypto.localOnly') }}</NTag>
    </header>

    <NAlert :title="t('crypto.warning.title')" type="warning">
      {{ t('crypto.warning.description') }}
    </NAlert>

    <section class="crypto-view__groups">
      <NCard :title="t('crypto.encrypt.title')" :bordered="false">
        <NSpace vertical>
          <NSelect v-model:value="encryptAlgorithm" :options="algorithmOptions" />
          <NAlert v-if="isLegacyCipher(encryptAlgorithm)" type="warning" :show-icon="false">
            {{ t('crypto.warning.legacyCipher') }}
          </NAlert>
          <NInput
            v-model:value="encryptInput"
            type="textarea"
            :autosize="{ minRows: 5, maxRows: 12 }"
            :placeholder="t('crypto.encrypt.inputPlaceholder')"
          />
          <NInput
            v-model:value="encryptPassphrase"
            type="password"
            show-password-on="click"
            :placeholder="t('crypto.fields.passphrase')"
          />
          <NAlert v-if="encryptError !== null" type="error">{{ encryptError }}</NAlert>
          <NSpace>
            <NButton type="primary" @click="runEncrypt">{{ t('crypto.actions.encrypt') }}</NButton>
            <NButton :disabled="encryptOutput === ''" @click="copy(encryptOutput)">
              {{ t('ui.copy') }}
            </NButton>
            <NButton :disabled="encryptOutput === ''" @click="moveToDecrypt">
              {{ t('crypto.actions.sendToDecrypt') }}
            </NButton>
          </NSpace>
          <NInput
            v-model:value="encryptOutput"
            readonly
            type="textarea"
            :autosize="{ minRows: 5, maxRows: 12 }"
            :placeholder="t('crypto.encrypt.outputPlaceholder')"
          />
        </NSpace>
      </NCard>

      <NCard :title="t('crypto.decrypt.title')" :bordered="false">
        <NSpace vertical>
          <NSelect v-model:value="decryptAlgorithm" :options="algorithmOptions" />
          <NAlert v-if="isLegacyCipher(decryptAlgorithm)" type="warning" :show-icon="false">
            {{ t('crypto.warning.legacyCipher') }}
          </NAlert>
          <NInput
            v-model:value="decryptInput"
            type="textarea"
            :autosize="{ minRows: 5, maxRows: 12 }"
            :placeholder="t('crypto.decrypt.inputPlaceholder')"
          />
          <NInput
            v-model:value="decryptPassphrase"
            type="password"
            show-password-on="click"
            :placeholder="t('crypto.fields.passphrase')"
          />
          <NAlert v-if="decryptError !== null" type="error">{{ decryptError }}</NAlert>
          <NSpace>
            <NButton type="primary" @click="runDecrypt">{{ t('crypto.actions.decrypt') }}</NButton>
            <NButton :disabled="decryptOutput === ''" @click="copy(decryptOutput)">
              {{ t('ui.copy') }}
            </NButton>
          </NSpace>
          <NInput
            v-model:value="decryptOutput"
            readonly
            type="textarea"
            :autosize="{ minRows: 5, maxRows: 12 }"
            :placeholder="t('crypto.decrypt.outputPlaceholder')"
          />
        </NSpace>
      </NCard>
    </section>

    <footer class="open-source-attribution">
      <span>{{ t('opensource.featureInspiredBy') }} IT Tools</span>
      <a :href="IT_TOOLS_PROJECT_URL" rel="noreferrer" target="_blank">
        {{ t('opensource.openOriginalProject') }}
      </a>
    </footer>
  </main>
</template>

<style scoped lang="scss">
.crypto-view {
  display: grid;
  gap: var(--page-gap);
  height: var(--app-viewport-height);
  min-height: 0;
  overflow: auto;
  padding: var(--page-padding);
  padding-inline: max(var(--page-padding), calc((100% - 74rem) / 2));

  header {
    align-items: center;
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    justify-content: space-between;

    h1,
    p {
      margin: 0;
    }

    p {
      color: var(--muted-color);
      margin-top: 0.25rem;
    }
  }

  &__groups {
    display: grid;
    gap: 1rem;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (width <= 820px) {
  .crypto-view__groups {
    grid-template-columns: 1fr;
  }
}
</style>
