<template>
    <div class="flex grow flex-col gap-8 overflow-y-auto p-4">
        <!-- Dictation -->
        <section class="shadow-card rounded-lg bg-white p-7 dark:bg-neutral-900">
            <h2 class="text-highlighted mb-6 text-sm font-semibold">Dictation Hotkey</h2>

            <div class="space-y-7">
                <!-- Hotkey -->
                <u-form-field label="Hotkey" description="Global shortcut to start/stop dictation from any app.">
                    <template #default>
                        <u-alert icon="i-lucide-info" color="info" variant="soft" title="Tip" class="mb-2">
                            <template #description>
                                If your keyboard has <u-kbd>Scroll Lock</u-kbd> or <u-kbd>Pause</u-kbd> keys, consider
                                binding the hotkey to one of them for single-key activation.
                            </template>
                        </u-alert>
                        <div class="flex items-center gap-2">
                            <div class="bg-muted flex w-fit min-w-96 items-center gap-1 rounded-md px-3 py-1.5 text-sm">
                                <u-kbd size="lg">Ctrl</u-kbd>
                                <span class="text-dimmed">+</span>
                                <u-kbd size="lg">`</u-kbd>
                            </div>
                            <!-- TODO: implement record shortcut UI -->
                            <u-button size="xs" variant="soft" disabled>Change</u-button>
                        </div>
                    </template>
                </u-form-field>

                <!-- Dicration hotkey mode -->
                <u-form-field
                    label="Dicration hotkey mode"
                    description="Hold: release key to stop. Toggle: press once to start, again to stop."
                >
                    <u-tabs
                        :model-value="hotkeyMode"
                        :items="hotkeyModeItems"
                        :content="false"
                        @update:model-value="hotkeyMode = $event as string"
                    />
                </u-form-field>

                <!-- Text input method -->
                <u-form-field label="Text input method">
                    <template #default>
                        <u-alert
                            icon="i-lucide-info"
                            color="info"
                            variant="soft"
                            :title="inputMethodTitle"
                            :description="inputMethodDescription"
                            class="mb-2"
                        />
                        <u-tabs
                            :model-value="settingsStore.textInputMethod"
                            :items="inputMethodItems"
                            :content="false"
                            @update:model-value="onInputMethodChange"
                        />
                    </template>
                </u-form-field>
            </div>
        </section>

        <!-- History -->
        <section class="shadow-card rounded-lg bg-white p-7 dark:bg-neutral-900">
            <h2 class="text-highlighted mb-4 text-sm font-semibold">History</h2>

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
        </section>

        <!-- Models -->
        <section class="shadow-card rounded-lg bg-white p-7 dark:bg-neutral-900">
            <h2 class="text-highlighted mb-4 text-sm font-semibold">Models</h2>

            <div v-if="audioStore.modelStatus" class="space-y-2">
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

                <p class="text-dimmed mt-2 text-xs">
                    Models directory:
                    <code class="bg-muted rounded px-1">{{ audioStore.modelStatus.modelsDir }}</code>
                </p>
            </div>
        </section>

        <!-- About -->
        <div class="text-dimmed border-default border-t pt-4 text-center text-xs">
            <p>Creo v0.1.0 · {{ platformLabel }}</p>
        </div>
    </div>
</template>

<script setup lang="ts">
    import IHold from '~/shared/ui/icons/ui/i-hold.vue';
    import ITap from '~/shared/ui/icons/ui/i-tap.vue';
    import type { TextInputMethod } from '~/entities/settings';
    import { useSettingsStore } from '~/entities/settings';
    import { useAudioStore } from '~/entities/audio';
    import { usePlatformStore } from '~/entities/platform';

    const settingsStore = useSettingsStore();
    const audioStore = useAudioStore();
    const platformStore = usePlatformStore();

    const hotkeyMode = ref('hold'); // TODO: persist via settings store

    const hotkeyModeItems = [
        { label: 'Hold', value: 'hold', icon: IHold },
        { label: 'Tap to start/end', value: 'toggle', icon: ITap },
    ];

    const inputMethodItems = [
        { label: 'Paste', value: 'paste' },
        { label: 'Type', value: 'type' },
    ];

    const inputMethodTitle = computed(() =>
        settingsStore.textInputMethod === 'paste' ? 'Replaces clipboard' : 'May be slow in heavy editors'
    );

    const inputMethodDescription = computed(() =>
        settingsStore.textInputMethod === 'paste'
            ? 'Dictated text replaces your current clipboard content.'
            : 'Each character triggers syntax highlighting repaint in VS Code, JetBrains, etc. Works fine in lightweight apps.'
    );

    const platformLabel = computed(() => {
        if (platformStore.isNativePlatform) return `Platform: ${platformStore.currentNativePlatform}`;
        return 'Platform: Web Browser';
    });

    function onInputMethodChange(value: string | number) {
        settingsStore.setTextInputMethod(String(value) as TextInputMethod);
    }

    function onRetentionChange(value: string | number) {
        const days = Math.max(1, Math.min(365, Number(value) || 30));
        settingsStore.setHistoryRetentionDays(days);
    }

    onMounted(async () => {
        if (platformStore.isNativePlatform) {
            await settingsStore.init();
            audioStore.checkModels();
        }
    });
</script>
