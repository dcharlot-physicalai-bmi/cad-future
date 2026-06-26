//! `physical-snn` — Spiking Neural Network for engineering design optimization.
//!
//! Implements Leaky Integrate-and-Fire (LIF) neuron dynamics with rate coding
//! for interfacing continuous engineering parameters with spike-based computation.

/// A single Leaky Integrate-and-Fire (LIF) neuron.
#[derive(Debug, Clone)]
pub struct SpikingNeuron {
    /// Current membrane potential (voltage).
    pub membrane_potential: f64,
    /// Firing threshold — neuron spikes when potential reaches this value.
    pub threshold: f64,
    /// Decay factor per timestep (0.0–1.0). Higher = faster leak.
    pub decay: f64,
    /// Number of timesteps the neuron is refractory after firing.
    pub refractory_period: u32,
    /// Remaining refractory countdown (0 = ready to integrate).
    refractory_remaining: u32,
}

impl SpikingNeuron {
    /// Create a new neuron with the given parameters.
    pub fn new(threshold: f64, decay: f64, refractory_period: u32) -> Self {
        Self {
            membrane_potential: 0.0,
            threshold,
            decay: decay.clamp(0.0, 1.0),
            refractory_period,
            refractory_remaining: 0,
        }
    }

    /// Reset the neuron state.
    pub fn reset(&mut self) {
        self.membrane_potential = 0.0;
        self.refractory_remaining = 0;
    }

    /// Integrate input current for one timestep. Returns `true` if the neuron fires.
    ///
    /// LIF dynamics:
    /// 1. If refractory, decrement counter and return false.
    /// 2. Leak: `V = V * (1 - decay)`
    /// 3. Integrate: `V = V + input`
    /// 4. If `V >= threshold`, fire (reset V, enter refractory), return true.
    pub fn step(&mut self, input: f64) -> bool {
        // Refractory period: neuron cannot fire.
        if self.refractory_remaining > 0 {
            self.refractory_remaining -= 1;
            return false;
        }

        // Leak.
        self.membrane_potential *= 1.0 - self.decay;

        // Integrate input.
        self.membrane_potential += input;

        // Threshold check.
        if self.membrane_potential >= self.threshold {
            self.membrane_potential = 0.0;
            self.refractory_remaining = self.refractory_period;
            return true;
        }

        false
    }
}

/// A layer of spiking neurons with a weight matrix.
#[derive(Debug, Clone)]
pub struct SnnLayer {
    /// Neurons in this layer.
    pub neurons: Vec<SpikingNeuron>,
    /// Weight matrix: `weights[i][j]` is the weight from input `j` to neuron `i`.
    pub weights: Vec<Vec<f64>>,
}

impl SnnLayer {
    /// Create a new layer with `num_neurons` neurons, each receiving `num_inputs` inputs.
    ///
    /// All weights are initialized to the given value.
    pub fn new(
        num_neurons: usize,
        num_inputs: usize,
        threshold: f64,
        decay: f64,
        refractory_period: u32,
        initial_weight: f64,
    ) -> Self {
        let neurons = (0..num_neurons)
            .map(|_| SpikingNeuron::new(threshold, decay, refractory_period))
            .collect();
        let weights = vec![vec![initial_weight; num_inputs]; num_neurons];
        Self { neurons, weights }
    }

    /// Number of neurons in this layer.
    pub fn len(&self) -> usize {
        self.neurons.len()
    }

    /// Whether this layer has no neurons.
    pub fn is_empty(&self) -> bool {
        self.neurons.is_empty()
    }

    /// Run one timestep. Takes inputs (one per input connection), returns spike
    /// output for each neuron (1.0 if fired, 0.0 otherwise).
    pub fn step(&mut self, inputs: &[f64]) -> Vec<f64> {
        self.neurons
            .iter_mut()
            .enumerate()
            .map(|(i, neuron)| {
                // Weighted sum of inputs.
                let current: f64 = self.weights[i]
                    .iter()
                    .zip(inputs.iter())
                    .map(|(w, x)| w * x)
                    .sum();
                if neuron.step(current) { 1.0 } else { 0.0 }
            })
            .collect()
    }

    /// Reset all neurons in this layer.
    pub fn reset(&mut self) {
        for n in &mut self.neurons {
            n.reset();
        }
    }
}

/// A multi-layer spiking neural network.
#[derive(Debug, Clone)]
pub struct SnnNetwork {
    /// Layers, from input to output.
    pub layers: Vec<SnnLayer>,
}

impl SnnNetwork {
    /// Create a network from a list of pre-built layers.
    pub fn new(layers: Vec<SnnLayer>) -> Self {
        Self { layers }
    }

    /// Create a simple feedforward network from layer sizes.
    ///
    /// `sizes` includes the input size as the first element, followed by
    /// the number of neurons in each subsequent layer.
    pub fn feedforward(
        sizes: &[usize],
        threshold: f64,
        decay: f64,
        refractory_period: u32,
        initial_weight: f64,
    ) -> Self {
        assert!(sizes.len() >= 2, "Need at least input size + one layer");
        let layers = sizes
            .windows(2)
            .map(|w| SnnLayer::new(w[1], w[0], threshold, decay, refractory_period, initial_weight))
            .collect();
        Self { layers }
    }

    /// Run one timestep through the entire network.
    ///
    /// Propagates inputs through each layer sequentially, returning the
    /// output spikes of the final layer.
    pub fn step(&mut self, inputs: &[f64]) -> Vec<f64> {
        let mut current = inputs.to_vec();
        for layer in &mut self.layers {
            current = layer.step(&current);
        }
        current
    }

    /// Reset all neurons in all layers.
    pub fn reset(&mut self) {
        for layer in &mut self.layers {
            layer.reset();
        }
    }
}

/// Encode a continuous value into a rate-coded spike train.
///
/// Produces a vector of `num_steps` booleans where the proportion of `true`
/// values approximates `value / max_rate`. Values are clamped to [0, max_rate].
///
/// Uses a simple deterministic threshold-crossing approach.
pub fn encode_rate(value: f64, max_rate: f64, num_steps: usize) -> Vec<bool> {
    if num_steps == 0 || max_rate <= 0.0 {
        return vec![false; num_steps];
    }

    let rate = (value / max_rate).clamp(0.0, 1.0);
    let mut spikes = Vec::with_capacity(num_steps);
    let mut accumulator = 0.0;

    for _ in 0..num_steps {
        accumulator += rate;
        if accumulator >= 1.0 {
            spikes.push(true);
            accumulator -= 1.0;
        } else {
            spikes.push(false);
        }
    }

    spikes
}

/// Decode a spike train back to a continuous value.
///
/// Returns the firing rate (proportion of `true` values), scaled by `max_rate`.
pub fn decode_rate(spikes: &[bool], max_rate: f64) -> f64 {
    if spikes.is_empty() {
        return 0.0;
    }
    let count = spikes.iter().filter(|&&s| s).count();
    (count as f64 / spikes.len() as f64) * max_rate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neuron_fires_at_threshold() {
        let mut neuron = SpikingNeuron::new(1.0, 0.0, 0);
        // Below threshold: no spike.
        assert!(!neuron.step(0.5));
        assert!(!neuron.step(0.4));
        // Now at 0.9, add 0.2 → 1.1 >= 1.0 → fire.
        assert!(neuron.step(0.2));
        // After firing, potential resets to 0.
        assert_eq!(neuron.membrane_potential, 0.0);
    }

    #[test]
    fn neuron_refractory_period() {
        let mut neuron = SpikingNeuron::new(0.5, 0.0, 2);
        // Fire.
        assert!(neuron.step(1.0));
        // Refractory: should not fire even with large input.
        assert!(!neuron.step(10.0));
        assert!(!neuron.step(10.0));
        // Refractory over, can fire again.
        assert!(neuron.step(1.0));
    }

    #[test]
    fn neuron_decay() {
        let mut neuron = SpikingNeuron::new(1.0, 0.5, 0);
        // Input 0.8, potential becomes 0.8.
        neuron.step(0.8);
        assert!(!neuron.step(0.0)); // Leaked: 0.8 * 0.5 = 0.4
        // Potential should be around 0.4.
        assert!((neuron.membrane_potential - 0.4).abs() < 1e-10);
    }

    #[test]
    fn neuron_reset() {
        let mut neuron = SpikingNeuron::new(1.0, 0.0, 5);
        neuron.step(2.0); // fires, enters refractory
        neuron.reset();
        assert_eq!(neuron.membrane_potential, 0.0);
        // Should be able to fire immediately (no refractory).
        assert!(neuron.step(2.0));
    }

    #[test]
    fn layer_step() {
        // 2 neurons, 2 inputs, weight = 1.0, threshold = 1.0.
        let mut layer = SnnLayer::new(2, 2, 1.0, 0.0, 0, 1.0);
        // Input [0.6, 0.5] → each neuron gets 0.6+0.5 = 1.1 >= 1.0 → both fire.
        let output = layer.step(&[0.6, 0.5]);
        assert_eq!(output, vec![1.0, 1.0]);
    }

    #[test]
    fn layer_subthreshold() {
        let mut layer = SnnLayer::new(2, 2, 2.0, 0.0, 0, 1.0);
        // Input [0.3, 0.3] → each neuron gets 0.6 < 2.0 → no fire.
        let output = layer.step(&[0.3, 0.3]);
        assert_eq!(output, vec![0.0, 0.0]);
    }

    #[test]
    fn network_propagation() {
        // 2 inputs → 3 hidden → 2 output
        let mut net = SnnNetwork::feedforward(&[2, 3, 2], 1.0, 0.0, 0, 0.5);
        // Large enough input to propagate through.
        let out = net.step(&[2.0, 2.0]);
        assert_eq!(out.len(), 2);
        // With weight=0.5, input layer gets 0.5*2+0.5*2=2.0 >= 1.0 → all 3 fire.
        // Output layer gets 0.5*1+0.5*1+0.5*1=1.5 >= 1.0 → both fire.
        assert_eq!(out, vec![1.0, 1.0]);
    }

    #[test]
    fn network_no_propagation_small_input() {
        let mut net = SnnNetwork::feedforward(&[2, 3, 2], 5.0, 0.0, 0, 0.1);
        let out = net.step(&[0.1, 0.1]);
        // Very small input * small weight = 0.02 < 5.0: nothing fires.
        assert_eq!(out, vec![0.0, 0.0]);
    }

    #[test]
    fn rate_coding_roundtrip() {
        let max_rate = 100.0;
        let original = 42.0;
        let num_steps = 1000;

        let spikes = encode_rate(original, max_rate, num_steps);
        let decoded = decode_rate(&spikes, max_rate);

        // Should be within 1% of original.
        assert!(
            (decoded - original).abs() < max_rate * 0.02,
            "decoded={decoded}, expected~{original}"
        );
    }

    #[test]
    fn rate_coding_zero() {
        let spikes = encode_rate(0.0, 100.0, 100);
        assert!(spikes.iter().all(|&s| !s));
        assert_eq!(decode_rate(&spikes, 100.0), 0.0);
    }

    #[test]
    fn rate_coding_max() {
        let spikes = encode_rate(100.0, 100.0, 100);
        assert!(spikes.iter().all(|&s| s));
        assert!((decode_rate(&spikes, 100.0) - 100.0).abs() < 1e-10);
    }

    #[test]
    fn rate_coding_clamps_above_max() {
        let spikes = encode_rate(200.0, 100.0, 50);
        // Clamped to max → all spikes.
        assert!(spikes.iter().all(|&s| s));
    }

    #[test]
    fn rate_coding_negative_clamped() {
        let spikes = encode_rate(-10.0, 100.0, 50);
        // Clamped to 0 → no spikes.
        assert!(spikes.iter().all(|&s| !s));
    }

    #[test]
    fn decode_empty() {
        assert_eq!(decode_rate(&[], 100.0), 0.0);
    }

    #[test]
    fn network_reset() {
        let mut net = SnnNetwork::feedforward(&[2, 2], 1.0, 0.0, 0, 1.0);
        net.step(&[1.0, 1.0]);
        net.reset();
        // After reset, all potentials should be zero.
        for layer in &net.layers {
            for neuron in &layer.neurons {
                assert_eq!(neuron.membrane_potential, 0.0);
            }
        }
    }
}
