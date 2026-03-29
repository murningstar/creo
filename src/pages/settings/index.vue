<template>
    <div class="flex grow flex-col gap-8 overflow-y-auto p-4">
        <RenameAssistant />

        <div class="grid items-start gap-8" style="grid-template-columns: repeat(auto-fit, minmax(24rem, 1fr))">
            <!-- Transcription Insert -->
            <section class="shadow-card rounded-lg bg-white p-7 dark:bg-neutral-900">
                <div>
                    <h2 class="text-highlighted text-sm font-semibold">Transcription Insert</h2>
                    <p class="text-dimmed mb-6 text-xs">
                        How text from dictation (<span class="italic"
                            >{{ settingsStore.assistantName }}, вписывай / готово</span
                        >
                        or hotkey) is inserted into the active app.
                    </p>

                    <div class="px-7">
                        <u-form-field label="Text input method">
                            <template #default>
                                <div class="mb-2">
                                    <div
                                        class="alert-slide"
                                        :class="{ open: settingsStore.textInputMethod === 'paste' }"
                                    >
                                        <div class="alert-slide-inner">
                                            <u-alert
                                                icon="i-lucide-info"
                                                color="info"
                                                variant="soft"
                                                title="Paste mode replaces clipboard content"
                                            >
                                                <template #description>
                                                    Works as if whole transcription was
                                                    <u-kbd>ctrl</u-kbd>+<u-kbd>v</u-kbd>'d.
                                                </template>
                                            </u-alert>
                                        </div>
                                    </div>
                                    <div
                                        class="alert-slide"
                                        :class="{ open: settingsStore.textInputMethod === 'type' }"
                                    >
                                        <div class="alert-slide-inner">
                                            <u-alert
                                                icon="i-lucide-info"
                                                color="info"
                                                variant="soft"
                                                title="Works as if all transcribed chars were typed one by one"
                                            >
                                                <template #description>
                                                    May be slow in heavy editors (VS Code, JetBrains) because of syntax
                                                    highlighting repaint. Works fine in lightweight apps.
                                                </template>
                                            </u-alert>
                                        </div>
                                    </div>
                                </div>
                                <u-tabs
                                    :model-value="settingsStore.textInputMethod"
                                    :items="inputMethodItems"
                                    :content="false"
                                    @update:model-value="onInputMethodChange"
                                />
                            </template>
                        </u-form-field>
                    </div>
                </div>
            </section>

            <!-- Dictation Hotkey -->
            <section class="shadow-card rounded-lg bg-white p-7 dark:bg-neutral-900">
                <div>
                    <h2 class="text-highlighted text-sm font-semibold">Dictation Hotkey</h2>
                    <p class="text-dimmed mb-6 text-xs">
                        Fallback for voice dictation command. Primary activation is via wake word.
                    </p>

                    <div class="space-y-10 px-7">
                        <!-- Hotkey -->
                        <u-form-field
                            label="Hotkey"
                            description="Global shortcut to start/stop dictation from any app."
                        >
                            <template #default>
                                <u-alert icon="i-lucide-info" color="info" variant="soft" title="Совет" class="mb-2">
                                    <template #description>
                                        Если у вас на клавиатуре есть кнопка
                                        <span class="inline-block whitespace-nowrap">
                                            <u-kbd>scroll lock</u-kbd>&nbsp;(&nbsp;<u-kbd variant="subtle">scrlk</u-kbd
                                            >&nbsp;)
                                        </span>
                                        или
                                        <span class="inline-block whitespace-nowrap">
                                            <u-kbd>pause break</u-kbd>&nbsp;(&nbsp;<u-kbd variant="subtle">pause</u-kbd
                                            >&nbsp;) </span
                                        >, поставьте активацию на них. Активировать одной кнопкой удобнее🙂
                                    </template>
                                </u-alert>
                                <HotkeyRecorder v-model="hotkey" />
                            </template>
                        </u-form-field>

                        <!-- Dictation hotkey mode -->
                        <u-form-field
                            label="Dictation hotkey mode"
                            description="Hold: release key to stop. Toggle: press once to start, again to stop."
                        >
                            <u-tabs
                                :model-value="hotkeyMode"
                                :items="hotkeyModeItems"
                                :content="false"
                                @update:model-value="hotkeyMode = $event as string"
                            />
                        </u-form-field>
                    </div>
                </div>
            </section>

            <!-- Right column: History + Models -->
            <div class="flex flex-col gap-8">
                <!-- History -->
                <section class="shadow-card rounded-lg bg-white p-7 dark:bg-neutral-900">
                    <div>
                        <h2 class="text-highlighted mb-4 text-sm font-semibold">History</h2>

                        <div class="px-7">
                            <u-form-field label="Keep history for">
                                <div class="flex items-center gap-2">
                                    <u-input
                                        :model-value="String(settingsStore.historyRetentionDays)"
                                        type="number"
                                        size="xs"
                                        class="w-24"
                                        @update:model-value="onRetentionChange"
                                    />
                                    <span class="text-muted text-sm">days</span>
                                </div>
                            </u-form-field>
                        </div>
                    </div>
                </section>

                <!-- Models -->
                <section class="shadow-card rounded-lg bg-white p-7 dark:bg-neutral-900">
                    <div>
                        <h2 class="text-highlighted text-sm font-semibold">Models</h2>
                        <p v-if="audioStore.modelStatus" class="text-dimmed mb-4 text-xs">
                            <code class="bg-muted rounded px-1">{{ audioStore.modelStatus.modelsDir }}</code>
                        </p>

                        <div v-if="audioStore.modelStatus" class="space-y-2 px-7">
                            <div
                                v-for="model in audioStore.modelStatus.models"
                                :key="model.filename"
                                class="bg-muted flex items-center justify-between rounded-md px-3 py-2 text-xs"
                            >
                                <div>
                                    <p class="font-medium">{{ model.name }}</p>
                                    <p class="text-dimmed">{{ model.filename }} ({{ model.sizeHint }})</p>
                                </div>
                                <u-badge
                                    :color="model.exists ? 'success' : 'error'"
                                    :variant="model.exists ? 'soft' : 'solid'"
                                    :label="model.exists ? 'OK' : 'Missing'"
                                    size="xs"
                                />
                            </div>
                        </div>
                    </div>
                </section>
            </div>
        </div>

        <!-- Dev Controls (dev mode only) -->
        <section
            v-if="isDev"
            class="shadow-card rounded-lg border border-dashed border-amber-300 bg-amber-50 p-7 dark:border-amber-700 dark:bg-amber-950/30"
        >
            <div>
                <h2 class="text-highlighted text-sm font-semibold">Dev Controls</h2>
                <p class="text-dimmed mb-4 text-xs">Development-only settings. Not visible in production.</p>

                <div class="space-y-4 px-7">
                    <u-form-field
                        label="Overlay: suppress devtools"
                        description="Remove Nuxt devtools and Vite error overlay from the indicator window."
                    >
                        <u-switch v-model="suppressOverlayDevtools" @update:model-value="onSuppressDevtoolsChange" />
                    </u-form-field>

                    <u-form-field
                        label="Overlay: click-through"
                        description="When ON, clicks pass through the overlay indicator to apps below."
                    >
                        <u-switch v-model="overlayClickThrough" @update:model-value="onClickThroughChange" />
                    </u-form-field>
                </div>
            </div>
        </section>

        <!-- About -->
        <div class="text-dimmed border-default mt-8 border-t pt-4 text-center text-xs">
            <p>Creo v0.1.0 · {{ platformStore.platformLabel }}</p>
        </div>
    </div>
</template>

<script setup lang="ts">
    import { emitTo } from '@tauri-apps/api/event';

    import { RenameAssistant } from '~/widgets/rename-assistant';
    import IHold from '~/shared/ui/icons/ui/i-hold.vue';
    import ITap from '~/shared/ui/icons/ui/i-tap.vue';
    import IPaste from '~/shared/ui/icons/ui/i-paste.vue';
    import IKeyboard from '~/shared/ui/icons/ui/i-keyboard.vue';
    import { HotkeyRecorder } from '~/features/hotkey-recorder';
    import type { KeyCombo } from '~/shared/ui/keystroke-recorder/model/types';
    import type { TextInputMethod, HotkeyMode } from '~/entities/settings';
    import { useSettingsStore } from '~/entities/settings';
    import { useAudioStore } from '~/entities/audio';
    import { usePlatformStore } from '~/entities/platform';

    const settingsStore = useSettingsStore();
    const audioStore = useAudioStore();
    const platformStore = usePlatformStore();

    // --- Dev controls ---
    const isDev = import.meta.dev;
    const suppressOverlayDevtools = ref(true);
    const overlayClickThrough = ref(true);

    async function onSuppressDevtoolsChange(value: boolean) {
        try {
            await emitTo('overlay', 'overlay-suppress-devtools', value);
        } catch (e) {
            console.warn('Failed to emit overlay-suppress-devtools:', e);
        }
    }

    async function onClickThroughChange(value: boolean) {
        try {
            await emitTo('overlay', 'overlay-set-click-through', value);
        } catch (e) {
            console.warn('Failed to emit overlay-set-click-through:', e);
        }
    }

    const hotkey = computed<KeyCombo | null>({
        get: () => settingsStore.hotkey,
        set: (value: KeyCombo | null) => settingsStore.setHotkey(value),
    });

    const hotkeyMode = computed<string>({
        get: () => settingsStore.hotkeyMode,
        set: (value: string) => settingsStore.setHotkeyMode(value as HotkeyMode),
    });

    const hotkeyModeItems = [
        { label: 'Hold', value: 'hold', icon: IHold },
        { label: 'Tap to start/end', value: 'toggle', icon: ITap },
    ];

    const inputMethodItems = [
        { label: 'Paste', value: 'paste', icon: IPaste },
        { label: 'Type', value: 'type', icon: IKeyboard },
    ];

    function onInputMethodChange(value: string | number) {
        settingsStore.setTextInputMethod(String(value) as TextInputMethod);
    }

    function onRetentionChange(value: string | number) {
        const days = Math.max(1, Math.min(365, Number(value) || 30));
        settingsStore.setHistoryRetentionDays(days);
    }
</script>

<style scoped>
    .alert-slide {
        display: grid;
        grid-template-rows: 0fr;
        transition: grid-template-rows 300ms ease;
    }

    .alert-slide.open {
        grid-template-rows: 1fr;
    }

    .alert-slide-inner {
        min-height: 0;
        overflow: hidden;
    }
</style>
