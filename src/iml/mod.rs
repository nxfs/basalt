/// IML for short stands for interactive main loop. For apps that are not purely just a user
/// interface frontend i.e. a music player. This is intended for use where the user wants to
/// maintain some control over the rendering process but still use basalt as a ui for a game.
/// It is still possible to use basalt without this and achieve the same things, but this
/// simplfies things around swapchain creation, and swapchain handling.

use parking_lot::Mutex;
use std::{
	collections::VecDeque,
	sync::{
		atomic::{self, AtomicUsize},
		Arc,
	},
	thread,
	time::{Duration, Instant},
};
use vulkano::{
	command_buffer::AutoCommandBufferBuilder,
	image::ImageUsage,
	instance::PhysicalDevice,
	swapchain::{self, Swapchain, SwapchainCreationError},
	sync::GpuFuture,
};
use vulkano::format::Format as VkFormat;
use vulkano::swapchain::ColorSpace as VkColorSpace;
use vulkano::command_buffer::CommandBufferUsage;
use vulkano::image::view::ImageView;
use interface::render::ItfRenderer;
use Basalt;

const SHOW_SWAPCHAIN_WARNINGS: bool = true;

pub(crate) struct IMLInitials {
    pub basalt: Arc<Basalt>,
    pub window_size: [u32; 2],
    pub vsync: bool,
}

pub struct BstIML {
    basalt: Arc<Basalt>,
    window_size: Mutex<[u32; 2]>,
    vsync: Mutex<bool>,
    fps: AtomicUsize,
}

impl BstIML {
    pub(crate) fn new(initials: IMLInitials) -> Result<Arc<Self>, String> {
        Ok(Arc::new(BstIML {
            basalt: initials.basalt,
            window_size: Mutex::new(initials.window_size),
            vsync: Mutex::new(initials.vsync),
            fps: AtomicUsize::new(0),
        }))
    }

    pub(crate) fn main_loop(&self) -> Result<(), String> {
        let mut win_size_x;
		let mut win_size_y;
		let mut frames = 0_usize;
		let mut last_out = Instant::now();
		let mut swapchain_ = None;
		let mut itf_resize = true;

		let pref_format_colorspace = vec![
			(VkFormat::B8G8R8A8Srgb, VkColorSpace::SrgbNonLinear),
			(VkFormat::B8G8R8A8Srgb, VkColorSpace::SrgbNonLinear),
		];
		
		let mut swapchain_format_op = None;
        let initial_swap_caps = self.basalt.swap_caps();

		for (a, b) in &pref_format_colorspace {
			for &(ref c, ref d) in &initial_swap_caps.supported_formats {
				if a == c && b == d {
					swapchain_format_op = Some((*a, *b));
					break;
				}
			}
			if swapchain_format_op.is_some() {
				break;
			}
		}

		let (swapchain_format, swapchain_colorspace) = swapchain_format_op
			.ok_or(format!(
				"Failed to find capatible format for swapchain. Avaible formats: {:?}",
				initial_swap_caps.supported_formats
			))?;
		println!("[Basalt]: Swapchain {:?}/{:?}", swapchain_format, swapchain_colorspace);

		let mut itf_renderer = ItfRenderer::new(self.basalt.clone());
		let mut previous_frame_future: Option<Box<dyn GpuFuture>> = None;
		let mut acquire_fullscreen_exclusive = false;

		'resize: loop {
            self.basalt.should_recreate_swapchain();

			let current_capabilities = self.basalt.surface_ref()
				.capabilities(
					PhysicalDevice::from_index(
                        self.basalt.surface_ref().instance(),
                        self.basalt.physical_device_index()
                    ).unwrap(),
				)
				.unwrap();

			let [x, y] = current_capabilities
				.current_extent
				.unwrap_or(self.basalt.surface_ref().window().inner_dimensions());
			win_size_x = x;
			win_size_y = y;
			*self.window_size.lock() = [x, y];

			if win_size_x == 0 || win_size_y == 0 {
				thread::sleep(Duration::from_millis(30));
				continue;
			}

			let present_mode = if *self.vsync.lock() {
				if initial_swap_caps.present_modes.relaxed {
					swapchain::PresentMode::Relaxed
				} else {
					swapchain::PresentMode::Fifo
				}
			} else {
				if initial_swap_caps.present_modes.mailbox {
					swapchain::PresentMode::Mailbox
				} else if initial_swap_caps.present_modes.immediate {
					swapchain::PresentMode::Immediate
				} else {
					swapchain::PresentMode::Fifo
				}
			};

			let mut min_image_count = current_capabilities.min_image_count;
			let max_image_count = current_capabilities.max_image_count.unwrap_or(0);

			if max_image_count == 0 || min_image_count + 1 <= max_image_count {
				min_image_count += 1;
			}

			swapchain_ = match match swapchain_
				.as_ref()
				.map(|v: &(Arc<Swapchain<_>>, _)| v.0.clone())
			{
				Some(old_swapchain) =>
					old_swapchain.recreate()
						.num_images(min_image_count)
						.format(swapchain_format)
						.dimensions([x, y])
						.usage(ImageUsage::color_attachment())
						.transform(swapchain::SurfaceTransform::Identity)
						.composite_alpha(self.basalt.options_ref().composite_alpha)
						.present_mode(present_mode)
						.fullscreen_exclusive(swapchain::FullscreenExclusive::AppControlled)
						.build(),
				None =>
					Swapchain::start(self.basalt.device(), self.basalt.surface())
						.num_images(min_image_count)
						.format(swapchain_format)
						.dimensions([x, y])
						.usage(ImageUsage::color_attachment())
						.transform(swapchain::SurfaceTransform::Identity)
						.composite_alpha(self.basalt.options_ref().composite_alpha)
						.present_mode(present_mode)
						.fullscreen_exclusive(swapchain::FullscreenExclusive::AppControlled)
						.build()
			} {
				Ok(ok) => Some(ok),
				Err(e) => match e {
					SwapchainCreationError::UnsupportedDimensions => continue,
					e => return Err(format!("Basalt failed to recreate swapchain: {}", e)),
				}
			};

			let (swapchain, images) =
				(&swapchain_.as_ref().unwrap().0, &swapchain_.as_ref().unwrap().1);
			let images: Vec<_> = images.into_iter().map(|i| ImageView::new(i.clone()).unwrap()).collect();
			let mut fps_avg = VecDeque::new();

			loop {
				previous_frame_future.as_mut().map(|future| future.cleanup_finished());
				let mut recreate_swapchain_now = false;

                if self.basalt.should_recreate_swapchain() {
                    itf_resize = true;
                    recreate_swapchain_now = true;
                }

				if recreate_swapchain_now {
					continue 'resize;
				}

				if acquire_fullscreen_exclusive {
					if swapchain.acquire_fullscreen_exclusive().is_ok() {
						acquire_fullscreen_exclusive = false;
						println!("Exclusive fullscreen acquired!");
					}
				}

				let duration = last_out.elapsed();
				let millis = (duration.as_secs() * 1000) as f32
					+ (duration.subsec_nanos() as f32 / 1000000.0);

				if millis >= 50.0 {
					let fps = frames as f32 / (millis / 1000.0);
					fps_avg.push_back(fps);

					if fps_avg.len() > 20 {
						fps_avg.pop_front();
					}

					let mut sum = 0.0;

					for num in &fps_avg {
						sum += *num;
					}

					let avg_fps = f32::floor(sum / fps_avg.len() as f32) as usize;
					self.fps.store(avg_fps, atomic::Ordering::Relaxed);
					frames = 0;
					last_out = Instant::now();
				}

				frames += 1;

				let (image_num, suboptimal, acquire_future) =
					match swapchain::acquire_next_image(
						swapchain.clone(),
						Some(::std::time::Duration::new(1, 0)),
					) {
						Ok(ok) => ok,
						Err(e) => {
							if SHOW_SWAPCHAIN_WARNINGS {
								println!(
									"Recreating swapchain due to acquire_next_image() error: \
									 {:?}.",
									e
								)
							}
							itf_resize = true;
							continue 'resize;
						},
					};

				let cmd_buf = AutoCommandBufferBuilder::primary(
					self.basalt.device(),
					self.basalt.graphics_queue_ref().family(),
					CommandBufferUsage::OneTimeSubmit
				)
				.unwrap();

				let (cmd_buf, _) = itf_renderer.draw(
					cmd_buf,
					[win_size_x, win_size_y],
					itf_resize,
					&images,
					true,
					image_num,
				);

				let cmd_buf = cmd_buf.build().unwrap();

				previous_frame_future = match match previous_frame_future.take() {
					Some(future) => Box::new(future.join(acquire_future)) as Box<dyn GpuFuture>,
					None => Box::new(acquire_future) as Box<dyn GpuFuture>,
				}
				.then_execute(self.basalt.graphics_queue(), cmd_buf)
				.unwrap()
				.then_swapchain_present(
					self.basalt.graphics_queue(),
					swapchain.clone(),
					image_num,
				)
				.then_signal_fence_and_flush()
				{
					Ok(ok) => Some(Box::new(ok)),
					Err(e) =>
						match e {
							vulkano::sync::FlushError::OutOfDate => {
								itf_resize = true;
								if SHOW_SWAPCHAIN_WARNINGS {
									println!(
										"Recreating swapchain due to \
										 then_signal_fence_and_flush() error: {:?}.",
										e
									)
								}
								continue 'resize;
							},
							_ => panic!("then_signal_fence_and_flush() {:?}", e),
						},
				};

				if suboptimal {
					itf_resize = true;
					continue 'resize;
				}

				itf_resize = false;

				if self.basalt.wants_exit() {
					break 'resize;
				}
			}
		}

		Ok(())
    }
}