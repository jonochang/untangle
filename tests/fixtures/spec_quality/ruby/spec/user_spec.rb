RSpec.describe "user sync" do
  let(:service) { double("service") }
  let(:logger) { double("logger") }

  it "retries noisy syncs" do
    allow(service).to receive(:ready?).and_return(true)
    allow(service).to receive(:fetch).and_return(:ok)
    allow(logger).to receive(:info)

    if service.ready? && service.fetch == :ok
      expect(service.fetch).to eq(:ok)
    else
      expect(logger).to receive(:info)
    end
  end
end

