use super::{ownership::Ownership, *};
use tokio::sync::Mutex;

#[derive(Copy, Clone)]
pub enum Person {
    Role(u64),
    User(u64),
}

#[derive(Copy, Clone)]
pub struct Repo {
    pub id: u64,
    // delete power
    pub admins: &'static [Person],
    /// power to `scour` and `retag` etc.
    pub chief: u64,
    pub deny_emoji: u64,
    /// clone url: https://bend-n:github_pat_…@github.com/…
    pub auth: &'static str,
    ownership: &'static LazyLock<Mutex<Ownership>>,
    // possibly posters?
}

use super::schematic::Schem;
use mindus::data::DataWrite;
impl Repo {
    pub fn auth(&self, member: &Member) -> bool {
        self.chief == member.user.id.get()
            || (self.admins.iter().any(|&x| match x {
                Person::Role(x) => member.roles.contains(&RoleId::new(x)),
                Person::User(x) => member.user.id.get() == x,
            }))
    }

    pub async fn own(&self) -> tokio::sync::MutexGuard<Ownership> {
        self.ownership.lock().await
    }

    pub fn schem(&self, dir: &str, x: MessageId) -> std::io::Result<mindus::Schematic> {
        std::fs::read(self.path(dir, x))
            .map(|x| mindus::Schematic::deserialize(&mut mindus::data::DataRead::new(&x)).unwrap())
    }

    pub fn path(&self, dir: &str, x: MessageId) -> std::path::PathBuf {
        self.repopath()
            .join(dir)
            .join(format!("{:x}.msch", x.get()))
    }

    pub fn gpath(&self, dir: &str, x: MessageId) -> std::path::PathBuf {
        Path::new(dir).join(format!("{:x}.msch", x.get()))
    }

    pub fn has(&self, dir: &str, x: MessageId) -> bool {
        self.path(dir, x).exists()
    }

    pub fn repopath(&self) -> std::path::PathBuf {
        Path::new("repos").join(format!("{:x}", self.id))
    }

    pub fn remove(&self, dir: &str, x: MessageId) {
        assert!(std::process::Command::new("git")
            .current_dir(self.repopath())
            .arg("rm")
            .arg("-q")
            .arg("-f")
            .arg(self.gpath(dir, x))
            .status()
            .unwrap()
            .success());
    }

    pub fn commit(&self, by: &str, msg: &str) {
        assert!(std::process::Command::new("git")
            .current_dir(self.repopath())
            .args(["commit", "-q", "--author"])
            .arg(format!("{by} <@designit>"))
            .arg("-m")
            .arg(msg)
            .status()
            .unwrap()
            .success());
    }

    pub fn push(&self) {
        assert!(std::process::Command::new("git")
            .current_dir(self.repopath())
            .arg("push")
            .arg("-q")
            .status()
            .unwrap()
            .success())
    }

    pub fn write(&self, dir: &str, x: MessageId, s: Schem) {
        _ = std::fs::create_dir(self.repopath().join(dir));
        let mut v = DataWrite::default();
        s.serialize(&mut v).unwrap();
        std::fs::write(self.path(dir, x), v.consume()).unwrap();
        self.add();
    }

    pub fn add(&self) {
        assert!(std::process::Command::new("git")
            .current_dir(self.repopath())
            .arg("add")
            .arg(".")
            .status()
            .unwrap()
            .success());
    }
}

macro_rules! decl {
    (
        [$($threaded:literal)+ $(,)?];
        $(
            $repo:literal => [
                $(
                    // xu64 => "dirname" : [label, label]
                    $ch:literal => $item:literal : [$($labels: expr),* $(,)?]
                ),+ $(,)?
            ];
        )+
    ) => {
        use emoji::to_mindustry::named::*;
        pub const THREADED: phf::Set<u64> = phf::phf_set! { $($threaded,)+ };
        pub const SPECIAL: phf::Map<u64, Ch> = phf::phf_map! {
            $($($ch => Ch { d: $item, labels: &[$($labels,)+], repo: $repo },)?)+
        };
    };
}
macro_rules! person {
    (&$x:literal) => {
        Person::Role($x)
    };
    ($x:literal) => {
        Person::User($x)
    };
}

macro_rules! repos {
    (
        $($repo_ident:ident $repo:literal => { admins: $admins:expr, chief: $chief:expr, deny_emoji: $deny:expr,} $(,)?),+
    ) => {

        $(static $repo_ident: LazyLock<Mutex<Ownership>> = LazyLock::new(|| Mutex::new(super::ownership::Ownership::new($repo)));)+
        pub static REPOS: phf::Map<u64, Repo> = phf::phf_map! {
            $($repo => Repo {
                id: $repo,
                admins: $admins,
                chief: $chief,
                deny_emoji: $deny,
                auth: include_str!(concat!("../../", $repo, ".auth")),
                ownership: &$repo_ident,
            },)+
        };
    };
}

repos! {
    DESIGN_IT 925674713429184564u64 => {
        admins: &[person!(&925676016708489227)],
        chief: 696196765564534825,
        deny_emoji: 1192388789952319499u64,
    },
    ACP 1110086242177142854u64 => {
        admins: &[person!(&1110439183190863913)],
        chief: 696196765564534825,
        deny_emoji: 1182469319641272370u64,
    }
}

decl! {
    [1129391545418797147u64];
    925674713429184564 => [
925721957209636914u64 => "cryofluid" : [CRYOFLUID, CRYOFLUID_MIXER],
925721791475904533u64 => "graphite" : [GRAPHITE, GRAPHITE_PRESS],
925721824556359720u64 => "metaglass" : [METAGLASS, KILN],
925721863525646356u64 => "phase-fabric" : [PHASE_FABRIC, PHASE_WEAVER],
927036346869104693u64 => "plastanium" : [PLASTANIUM, PLASTANIUM_COMPRESSOR],
925736419983515688u64 => "pyratite" : [PYRATITE, PYRATITE_MIXER],
925736573037838397u64 => "blast-compound" : [BLAST_COMPOUND, BLAST_MIXER],
927793648417009676u64 => "scrap" : [DISASSEMBLER, SCRAP],
1198556531281637506u64 => "spore-press" : [OIL, SPORE_PRESS],
1200308146460180520u64 => "oil-extractor" : [OIL, OIL_EXTRACTOR],
1200301847387316317u64 => "rtg-gen" : [POWER, RTG_GENERATOR],
1200308292744921088u64 => "cultivator" : [SPORE_POD, CULTIVATOR],
1200305956689547324u64 => "graphite-multipress" : [GRAPHITE, MULTI_PRESS],
1200306409036857384u64 => "silicon-crucible" : [SILICON, SILICON_CRUCIBLE],
1198555991667646464u64 => "coal" : [COAL, COAL_CENTRIFUGE],
925721763856404520u64 => "silicon" : [SILICON, SILICON_SMELTER],
925721930814869524u64 => "surge-alloy" : [SURGE_ALLOY, SURGE_SMELTER],
1141034314163826879u64 => "defensive-outpost" : [""],
949529149800865862u64 => "drills" : [PRODUCTION],
925729855574794311u64 => "logic-schems" : [MICRO_PROCESSOR],
1185702384194818048u64 => "miscellaneous" : ["…"],
1018541701431836803u64 => "combustion-gen" : [POWER, COMBUSTION_GENERATOR],
927480650859184171u64 => "differential-gen" : [POWER, DIFFERENTIAL_GENERATOR],
925719985987403776u64 => "impact-reactor" : [POWER, IMPACT_REACTOR],
949740875817287771u64 => "steam-gen" : [POWER, STEAM_GENERATOR],
926163105694752811u64 => "thorium-reactor" : [POWER, THORIUM_REACTOR],
973234467357458463u64 => "carbide" : [CARBIDE, ""],
1198527267933007893u64 => "erekir-defensive-outpost" : [""],
973236445567410186u64 => "fissile-matter" : [FISSILE_MATTER, ""],
1147887958351945738u64 => "electrolyzer" : [HYDROGEN, OZONE, ""],
1202001032503365673u64 => "nitrogen" : [NITROGEN, ""],
1202001055349477426u64 => "cyanogen" : [CYANOGEN, ""],
1096157669112418454u64 => "mass-driver" : ["…", PLANET],
973234248054104115u64 => "oxide" : [OXIDE, ""],
973422874734002216u64 => "erekir-phase" : [PHASE_FABRIC, ""],
973369188800413787u64 => "ccc" : ["", POWER],
1218453338396430406u64 => "neoplasia-reactor": ["", POWER],
1218453292045172817u64 => "flux-reactor": ["", POWER],
1218452986788053012u64 => "pyrolisis-gen": ["", POWER],
1147722735305367572u64 => "silicon-arc" : [SILICON, ""],
974450769967341568u64 => "erekir-surge" : [SURGE_ALLOY, ""],
973241041685737532u64 => "erekir-units" : ["[#ff9266][]"],
1158818171139133490u64 => "unit-core" : [UNITS, CORE_NUCLEUS],
1158818324210274365u64 => "unit-delivery" : [UNITS, FLARE],
1158818598568075365u64 => "unit-raw" : [UNITS, PRODUCTION],
1142181013779398676u64 => "unit-sand" : [UNITS, SAND],
1222270513045438464u64 => "bore": [PRODUCTION],
1226407271978766356u64 => "pulveriser": [PULVERIZER, SAND],
1277138620863742003u64 => "melter": [MELTER, SLAG],
1277138532355543070u64 => "separator": [SEPARATOR, SCRAP],
    ];
1110086242177142854u64 => [
        1276759410722738186u64 => "schems": ["plague"]
    ];
}

macro_rules! chief {
    ($c:ident) => {{
        let repo = repos::REPOS[&$c.guild_id().unwrap().get()];
        if repo.chief != $c.author().id.get() {
            poise::send_reply(
                $c,
                poise::CreateReply::default()
                    .content(format!(
                        "access denied. only the chief <@{}> can use this command.",
                        repo.chief,
                    ))
                    .allowed_mentions(CreateAllowedMentions::default().empty_users().empty_roles()),
            )
            .await?;
            return Ok(());
        }
        repo
    }};
}

pub(crate) use chief;
