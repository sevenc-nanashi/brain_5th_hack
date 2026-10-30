import { View, Text, ActionHandler } from "@pocketjs/framework/solid/components";
import { BTN } from "@pocketjs/framework/input";
import { count, increment, reset } from "./App";

export default function App() {
  return (
    <View class="w-[800px] h-full flex-col bg-[#f3f0e8] p-10 gap-6">
      <ActionHandler button={BTN.CROSS} onPress={increment} />
      <ActionHandler button={BTN.TRIANGLE} onPress={reset} />
      <Text class="text-2xl text-[#35433d]">PocketJS / Brain</Text>
      <View class="flex-1 flex-col justify-center bg-[#183e32] rounded-[16px] p-8 gap-4">
        <Text class="text-2xl text-[#bad7c8]">COUNTER</Text>
        <Text class="text-5xl text-white">{count()}</Text>
      </View>
      <Text class="text-2xl text-[#35433d]">Enter: +1 / R: Reset / Esc: Exit</Text>
    </View>
  );
}
