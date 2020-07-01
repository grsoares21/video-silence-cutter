import { Machine, assign, interpret } from "xstate";

type SilenceContext = {
  subsequentialSamplesInSilence: number;
  subsequentialSamplesInNoise: number;
};

export default (attackTime: number, releaseTime: number) =>
  interpret(
    Machine<SilenceContext>({
      id: "SilenceTrimmer",
      initial: "Silence",
      context: {
        subsequentialSamplesInSilence: 0,
        subsequentialSamplesInNoise: 0,
      },
      states: {
        Noisy: {
          on: {
            SAMPLE_SILENCE: "PotentialSilenceStart",
            SAMPLE_NOISY: "Noisy",
          },
        },
        PotentialSilenceStart: {
          on: {
            SAMPLE_SILENCE: [
              {
                target: "Silence",
                cond: (context) => {
                  return context.subsequentialSamplesInSilence > attackTime;
                },
                actions: assign({
                  subsequentialSamplesInSilence: 0,
                }),
              },
              {
                target: "PotentialSilenceStart",
                actions: assign({
                  subsequentialSamplesInSilence: (context, event) =>
                    context.subsequentialSamplesInSilence + 1,
                }),
              },
            ],
            SAMPLE_NOISY: {
              target: "Noisy",
              actions: assign({
                subsequentialSamplesInSilence: 0,
              }),
            },
          },
        },
        PotentialSilenceFinish: {
          on: {
            SAMPLE_SILENCE: {
              target: "Silence",
              actions: assign({
                subsequentialSamplesInNoise: 0,
              }),
            },
            SAMPLE_NOISY: [
              {
                target: "Noisy",
                cond: (context) => {
                  return context.subsequentialSamplesInNoise > releaseTime;
                },
                actions: assign({
                  subsequentialSamplesInNoise: 0,
                }),
              },
              {
                target: "PotentialSilenceFinish",
                actions: assign({
                  subsequentialSamplesInNoise: (context, event) =>
                    context.subsequentialSamplesInNoise + 1,
                }),
              },
            ],
          },
        },
        Silence: {
          on: {
            SAMPLE_SILENCE: "Silence",
            SAMPLE_NOISY: "PotentialSilenceFinish",
          },
        },
      },
    })
  );
